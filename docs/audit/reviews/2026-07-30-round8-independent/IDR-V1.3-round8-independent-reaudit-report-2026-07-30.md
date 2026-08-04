# IDR V1.3 Round 8 独立复审报告

审计日期：2026-07-30  
审计对象：`IDR-V1.3-trust-chain-closure-round8-2026-07-30.zip`  
外层 ZIP SHA-256：`7724ec3600cb6aa57249ca2b121b4503043b142ff14d3352e606531d619be7ba`

## 1. 总体结论

Round 8 对 Round 7 的五项 P0 做了实质整改，其中四项已经按代码证据闭合，一项只完成了**数据库布局隔离**，尚未完成**密码学信任域隔离**。

本轮没有通过第一次独立干净复审。当前结论为：

```text
ROUND8_FIRST_CLEAN_INDEPENDENT_REVIEW = NO

P0 = 3
P1 = 14
P2 = 4

IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

Round 8 已经解决了上一轮最直接的 API 与语义旁路，但仍有三项会破坏“唯一、可证明、可持久、可恢复、不可绕过”生产信任链的核心问题：

1. `trust_domain` 没有进入签名 Command/Proof/Public Ref/Checkpoint 身份，同一签名 Proof 可以在两个隔离 schema 中分别消费；
2. Action 与 Action Admission 没有权威有效期，已经过期的用户授权可以在很久以后被间接复用来创建 Execution Reservation；
3. PostgreSQL 中权威 Contract 和专用投影仍可被原地修改，启动完整性检查不会重新计算 Contract digest，也没有数据库不可变更新/删除门禁。

因此，Round 8 不能计为第一轮无信任边界 P0 的独立复审。

---

## 2. 审计范围和独立验证

### 2.1 压缩包完整性

独立验证结果：

- 外层 ZIP SHA-256 与提交的 `.sha256` 文件一致；
- ZIP 条目：1,020；
- 路径穿越：0；
- 符号链接：0；
- 解压后文件总数：840；
- `SHA256SUMS` 清单项：839；
- 包内摘要：839/839 PASS。

### 2.2 本环境独立执行

已独立执行：

- TypeScript `npm ci --offline`：PASS；
- TypeScript tests：12/12 PASS；
- TypeScript typecheck：PASS；
- Python tests：9/9 PASS。

本审计环境没有 `cargo`、`rustc` 和 PostgreSQL 服务，因而未独立重跑 Rust/PostgreSQL 测试。包内维护者验证日志记录了 Rust、Clippy、rustfmt、PostgreSQL 与 Aegis 测试通过；这些日志已通过包内 SHA 清单验证，但仍属于维护者执行证据，而非本环境动态复现。

---

## 3. Round 7 五项 P0 关闭矩阵

| Round 7 P0 | Round 8 结论 | 证据摘要 |
|---|---|---|
| production + test-support 重新开放 raw API | **关闭** | `idr-runtime/src/lib.rs:6-10` 和 `idr-store/src/lib.rs` 对危险 feature 组合执行 compile error；raw helpers 仅在 `shadow-mode + test-support` 下可见 |
| 独立发布门禁未生效 | **关闭** | `postgres_authority.rs:1181-1239` 分别检查 Trust Root、Authorization、Execution、Human Model 门禁，并按 command family 拒绝 |
| Shadow 是调用者可选的信任旁路 | **部分关闭** | 运行时 mode selector 已删除，Shadow/Production 分为不同构造器和 schema；但 `trust_domain` 未进入 Command/Proof 的签名身份，跨 schema 重放仍成立 |
| Proof 未绑定完整 command target | **关闭** | `trust_chain.rs:271-308,451-470` 将 command ID、run、expected version、principal、tenant/scope/purpose/policy、correlation、causation 和完整 command 纳入 proof subject |
| Human Model lifecycle/impact 可由调用者升级 | **关闭** | `trust_chain.rs:1587-1668` 从 Promotion outcome 和 source Candidate 内部派生 lifecycle/impact；migration 0003 `153-226` 增加数据库二次校验 |

结论：**4/5 完全关闭，1/5 部分关闭。**

---

# 4. P0 问题

## P0-01：Trust Domain 只进入持久层，没有进入签名身份

### 代码证据

`IdrCommandEnvelopeV1` 与 `UnsignedCommandEnvelopeV1` 不包含 `trust_domain` 或 `environment_id`：

- `crates/idr-runtime/src/trust_chain.rs:271-308`
- `crates/idr-runtime/src/trust_chain.rs:451-470`

`ProductionProofClaimsV1` 和 `ExpectedProductionProofV1` 同样不包含信任域：

- `crates/idr-protocol/src/production.rs:248-268`
- `crates/idr-protocol/src/production.rs:462-473`

Proof 验证只比较 issuer、subject、digest、audience、tenant、scope、purpose 和 policy：

- `crates/idr-protocol/src/production.rs:573-620`

Shadow 与 Production 构造器虽然强制不同 schema，但都接受调用方传入的普通 `PostgresSecurityContextV1`，没有要求 domain-specific audience、domain-specific Trust Root 或 domain-specific issuer registry：

- `crates/idr-runtime/src/postgres_authority.rs:132-158`
- `crates/idr-runtime/src/postgres_authority.rs:247-321`

Proof ID 和 nonce 的防重放记录位于各自 schema 内部：

- `crates/idr-runtime/src/postgres_authority.rs:1587-1637`

外部 checkpoint 也只有 tenant，没有 trust domain/environment：

- `crates/idr-runtime/src/postgres_authority.rs:73-92`

In-memory backend 的索引键只是 `tenant_ref`：

- `crates/idr-runtime/src/postgres_authority.rs:98-124`

### 可复现方式

不需要解除 Production 门禁即可证明基本缺陷：

1. 建立 `idr_shadow_a` 与 `idr_shadow_b` 两个 Shadow schema；
2. 给两套 Orchestrator 使用相同 Trust Root、audience 和 expected issuers；
3. 构造一份完整 `IdrCommandEnvelopeV1` 和一组签名 Proof；
4. 先提交到 schema A，再将完全相同的 command/proofs 提交到 schema B；
5. 两个 schema 的 proof consumption 表相互独立，而签名内容没有环境身份，因此两边均可接受。

Production 门禁未来解除后，同样的结构允许 Shadow Proof 在配置相同 Trust Root/audience 时进入 Production schema。

### 影响

- Proof 的“一次消费”只在单个 schema 内成立；
- Shadow 结果和 Production 结果没有密码学域隔离；
- Public Record Ref、Command Receipt 和 Checkpoint 不能单独证明它属于哪个信任域；
- 共享 anchor backend 时，不同域还可能相互覆盖或造成拒绝服务。

### 推荐修复

新增不可省略的 `TrustDomainRefV1`/`EnvironmentIdV1`，并纳入：

- Unsigned Command digest；
- Production Proof claims 和 expected proof；
- Trust Root policy；
- audience；
- `AuthoritativeRecordRefV1`；
- `IdrCommandReceiptV1`；
- Audit checkpoint key、payload 和签名；
- Proof/nonce 的全局消费键。

同时要求 Shadow 和 Production 使用不同 audience、不同 Trust Root namespace 和不可交叉的 issuer policy，并增加“同一 Proof 跨两个 schema 重放”的攻击测试。

---

## P0-02：Action 与 Action Admission 没有有效期，过期授权可被延迟复用

### 代码证据

Master Spec 要求所有权威对象包含有效期，并要求 Action Admission 在同一事务中验证 Action 未过期，同时输出 `issued_at / valid_until`：

- `IDR-V1.3-Trust-Chain-Closure-Master-Spec.md:225`
- `IDR-V1.3-Trust-Chain-Closure-Master-Spec.md:704-741`

但 `CandidateSubmissionV1` 没有 `valid_until`：

- `crates/idr-protocol/src/production.rs:142-158`

`AuthoritativeRecordV1` 只有 `issued_at`，没有 `valid_until`：

- `crates/idr-runtime/src/trust_chain.rs:111-127`

Action Admission payload 记录 Action/provider/owner/operation/parameter/request/idempotency 和 Trust Root version，却不记录：

- exact Authorization ref；
- Authorization expiry；
- Admission valid_until；
- Action valid_until。

证据：

- `crates/idr-runtime/src/trust_chain.rs:1168-1219`

`ReserveExecution` 只检查：

- Action 和 Admission current；
- exact bindings；
- attempt；
- `lease_until > trusted_now`。

它没有重新检查 Action、Action Admission 或原 Exact Authorization 是否仍在有效期：

- `crates/idr-runtime/src/trust_chain.rs:1221-1303`

### 可复现方式

1. 在时间 150 使用 `expires_at=200` 的 Exact Authorization Proof 完成 `AdmitAction`；
2. Admission 成为 current authoritative record；
3. 等待到时间 1,000；
4. 为 `ReserveExecution` 获取一份新的 ExecutionPermit Proof，并设置未来 lease；
5. 当前 Runtime 只验证新的 ExecutionPermit Proof 和 Admission 引用，不重新验证原 Authorization 的 expiry；
6. Reservation 可以建立。

### 影响

- 用户只在短窗口内授权的 Action，可以在授权失效后很久才执行；
- 撤销/时效语义停留在 Admission 创建时，而不是实际执行前；
- Permit 没有达到 Master Spec 所要求的 exact Authorization 绑定。

### 推荐修复

- Action、ActionDerivation、ActionAdmission 全部增加 `valid_from/valid_until`；
- Admission 持久化 exact Authorization proof ID/ref、expiry 和 policy validity；
- Reservation 和 Dispatch 事务内重新验证：Action current/nonexpired、Admission nonexpired、Authorization nonexpired/nonrevoked、Trust Root current；
- Permit payload 显式绑定 exact Authorization 和 Admission 的有效期；
- 增加“授权在 Admit 后、Reserve 前过期”和“Admission 在 Permit delivery 前过期”的测试。

---

## P0-03：权威 Contract 和专用投影可以被原地篡改，完整性检查无法发现

### 代码证据

`idr_contract_records` 把权威对象保存在普通可更新的 `jsonb record` 中：

- `crates/idr-store/migrations/0001_idr_v13_trust_chain.sql:47-61`

Record digest 的确在签发时覆盖权威对象全部主要字段：

- `crates/idr-runtime/src/trust_chain.rs:1729-1783`
- `crates/idr-runtime/src/trust_chain.rs:1786-1863`

但持久化后：

- 没有 `BEFORE UPDATE/DELETE` 不可变触发器；
- 没有 REVOKE/角色约束脚本；
- 没有在加载或启动时重新计算 `record` 与 `record_digest`。

`verify_database_integrity()` 只验证 audit hash chain 和 Run projection digest：

- `crates/idr-runtime/src/postgres_authority.rs:569-685`

它不扫描 `idr_contract_records.record`，也不验证 specialized projection tables。

Human Model materialization trigger 只在 INSERT 时执行，UPDATE 不受约束：

- `crates/idr-store/migrations/0003_round8_trust_domains_and_human_model.sql:153-226`

### 可复现方式

在一个已有 Human Model Assertion 的数据库中，以应用数据库凭据执行：

```sql
UPDATE idr_contract_records
SET record = jsonb_set(record, '{payload,value_digest}', to_jsonb(repeat('f', 64)))
WHERE candidate_kind = 'human_model_assertion';

UPDATE idr_human_model_assertions
SET assertion = jsonb_set(assertion, '{payload,value_digest}', to_jsonb(repeat('f', 64)))
WHERE assertion_record_id = '<target>';
```

保持 `record_digest`、audit event 和 Run projection 不变，然后重启 Orchestrator。

当前 `verify_database_integrity()` 不会重新计算该 Contract digest，也不验证 specialized assertion content；启动校验可以通过。

### 影响

- 数据库凭据泄漏、运维误操作或 SQL 注入可无痕修改权威事实；
- 外部 anchor 只锚定 audit chain 中的旧 ref/digest，不能证明当前 JSON 内容仍与 digest 一致；
- Human Model、Outcome、Receipt 等专用投影可能与权威 Contract 分叉。

### 推荐修复

- 权威表建立 append-only 数据库角色，Runtime 角色禁止 UPDATE/DELETE；
- 增加不可变触发器，只有显式 forward revision procedure 可写；
- 保存 canonical bytes 或可由 PostgreSQL/Runtime稳定重算的内容 digest；
- 启动、加载和 replay 时逐条重算 Contract digest；
- 对 proof envelopes、dependencies、current pointers、execution、outcome、HM specialized tables 做交叉一致性扫描；
- audit checkpoint 覆盖 canonical record set root/Merkle root，而不只是 event payload；
- 增加 Contract JSON、specialized projection 和 current pointer 篡改攻击测试。

---

# 5. P1 问题

## P1-01：没有生产 WORM/KMS anchor 和 checkpoint signer

`AuditCheckpointAnchorBackendV1` 是公共可实现 trait，`is_production_durable()` 是实现方自报；checkpoint 本身没有签名。包内文档也明确将其列为未完成。

证据：`postgres_authority.rs:73-92`；`docs/trust-chain-closure/REMAINING-RISKS.md:8-13`。

## P1-02：Trust Root provider 不是受保护控制服务

`CurrentTrustRootProviderV1` 是进程内公共 trait，生产 Trust Root 的分发、轮换、授权和审计仍依赖注入实现。

证据：`postgres_authority.rs:33-70`；Remaining Risks 第 2 项。

## P1-03：Trust Root rotation/revocation 与数据库事务不原子

事务开始后从进程内 provider 取得 snapshot，再验证并提交；Trust Root 在 snapshot 读取后被撤销时，数据库事务没有受同一一致性机制保护。

证据：`postgres_authority.rs:785-821`。

## P1-04：Checkpoint 在数据库 commit 后发布

`transact_inner()` 已提交后，`handle()` 才调用 external anchor publish。进程在二者之间崩溃会产生 unanchored tail。

证据：`postgres_authority.rs:1096-1108`。

## P1-05：Permit 仍不是 Provider-facing signed capability

数据库内有持久 execution projection，但 Provider 侧没有携带 Runtime/Store 签名、可独立验证的 Permit wire object。包内也明确承认。

证据：Master Spec `800-817`；Remaining Risks 第 4 项。

## P1-06：Response 只有 send consumption，没有 delivery/fan-out receipt

`ConsumeResponseSend` 消费 policy/admission proof 和 send nonce，但未实现 Adapter delivery acknowledgement、fan-out per-recipient receipt、失败恢复和 reconciliation。

证据：`trust_chain.rs:180-186,1082-1109`；Remaining Risks 第 5 项。

## P1-07：Capability/Authority/Policy issuer separation 仍不完整

所有 expected issuers 由单个 `PostgresSecurityContextV1` map 注入，缺少受保护 Registry、职责隔离和独立发布链。

证据：`postgres_authority.rs:132-158`；Remaining Risks 第 6 项。

## P1-08：Outcome Runtime 仍是最小单观察模型

Outcome 校验只要求 Receipt digest、observed value digest 和一段 observation 字符串；没有结构化 conflicts、attribution、observation scheduling、多个独立观察和 stale-action disclosure。

证据：`trust_chain.rs:2312-2322`；Remaining Risks 第 7 项。

## P1-09：Human Model “独立证据数”只是调用方数字

`promote_provisional` 只检查 `independent_evidence_count >= 2`，没有验证两个不同 Evidence Root。敏感属性分类、retention、decay、删除和生产 query API 也未完成。

证据：`trust_chain.rs:2324-2346`；Remaining Risks 第 8 项。

## P1-10：新 Action 取消旧 execution 后无法创建下一 Reservation

记录新 Action 时，旧的未终态 execution 被设为 `Cancelled`：

- `trust_chain.rs:1152-1164`

但下一次 `ReserveExecution` 只允许旧状态为 `Failed/Rejected/Expired`，不接受 `Cancelled`：

- `trust_chain.rs:1253-1269`

这会让安全重规划后的新 Action lineage 被旧 execution projection 锁死。

## P1-11：部分非 Candidate 命令字段没有大小上限

Candidate payload 有 1 MiB、32 层和 10,000 node 限制，但：

- `rendered_bytes: Vec<u8>` 没有上限；
- `idempotency_key: String` 只检查非空；
- 这些字段进入 proof digest、audit/outbox/数据库路径。

证据：`trust_chain.rs:180-206,345-380,1175-1177,1246-1250`；`production.rs:190-200,746-758`。

## P1-12：Trust-domain/完整性启动扫描未覆盖所有权威表

`verify_trust_domain()` 只检查 runs、contract records、proof consumptions、audit、outbox、receipts；没有覆盖 proof envelopes、checkpoints、execution specialized tables、outcome/HM specialized tables。

证据：`postgres_authority.rs:497-523`。

## P1-13：旧数据迁移为 `unclassified`，但缺少生产迁移证明工具

Migration 0003 通过 `unclassified` 列和启动拒绝实现 fail-closed，但包内没有面向真实历史数据的受签名 replay/migration 工具和完整兼容语料。包内也承认 migration corpus 未达到发布门槛。

## P1-14：Aegis Production Adapter 与连续两轮独立复审仍未完成

Aegis Production adapter 继续 compile-blocked；本轮又出现三个新 P0，因此不能计入第一轮 clean review。

---

# 6. P2 问题

## P2-01：公共 Ref/Receipt 不携带 Trust Domain

`AuthoritativeRecordRefV1` 与 `IdrCommandReceiptV1` 没有 domain/environment，跨系统日志或缓存中无法仅凭对象判断来源域。

证据：`trust_chain.rs:76-101,881-891`。

## P2-02：Human Model correction revision 使用 `saturating_add`

理论上的 `u64::MAX` 修订不会报 overflow，而会继续产生同 revision，最终依赖数据库错误拒绝。

证据：`trust_chain.rs:1671-1685`。

## P2-03：Round 8 文档对 Shadow closure 表述过强

文档声称持久化 trust domain 已关闭 Shadow P0，但实际只关闭了 schema/表级混用，没有关闭 signed proof 的跨环境重放。

## P2-04：没有 Git provenance

包内 SHA 清单可以固定快照，但没有 branch、commit、author、review history 和 signed tag，不能替代可追溯源码历史。

---

# 7. 已正确实现并应保留的部分

Round 8 以下整改已经达到较高质量：

1. 危险 feature 组合 compile-fail；
2. Shadow/Production 不再由 runtime 参数切换；
3. 独立发布门禁按 command family 生效；
4. 完整 Unsigned Command proof target；
5. authoritative transition、authority token 和 PostgreSQL mutation entry 封闭；
6. Decision→Action option/operation/parameter 精确派生；
7. subject、turn、tenant、scope、purpose、policy lineage 检查；
8. successor 自动递归失效；
9. terminal Run 执行门禁；
10. owner/provider caller binding；
11. operation-level idempotency；
12. Receipt request digest 和 execution identity binding；
13. HM Assertion 由 Candidate/Promotion 内部 materialize；
14. TypeScript/Python generated conformance 和 strict parsing。

这些部分不应因下一轮重构而退回。

---

# 8. Round 9 推荐实施顺序

Round 9 不应继续扩展新 Contract。只处理以下顺序：

## Phase 1：密码学信任域闭合

- domain/environment 进入 Command、Proof、Trust Root、Public Ref、Receipt、Checkpoint；
- domain-specific audience/issuer/key namespace；
- cross-schema/cross-domain Proof replay 攻击测试。

## Phase 2：权威有效期闭合

- 所有 authoritative contract 强制 valid_from/valid_until；
- ActionAdmission 绑定 exact Authorization ref/expiry；
- Reserve/Deliver/Dispatch 事务内重验；
- stale authorization/admission attack tests。

## Phase 3：Store 内容不可变与全量校验

- DB role separation；
- append-only triggers；
- canonical record bytes/digest replay；
- specialized projection cross-check；
- record-set root 进入 external checkpoint。

## Phase 4：处理 P1 可靠性

- 解决 Cancelled execution 阻塞新 Action reservation；
- command byte/string budgets；
- Trust Root transaction coordination；
- response delivery receipts；
- provider-facing signed Permit。

## Phase 5：独立复审门槛

必须满足：

```text
P0 = 0
security/integrity P1 = 0
cross-domain replay suite = PASS
stale authorization suite = PASS
record tamper/restart suite = PASS
first clean independent review = PASS
second consecutive clean independent review = PASS
```

---

# 9. 最终判定

Round 8 已经从“公共 API 可直接绕过”的阶段，进入“核心路径基本封闭，但信任域、时效和持久内容完整性仍未闭合”的阶段。这是重要进步，但三项剩余 P0 都位于生产信任根本身，而不是外围部署便利性。

因此，本轮不能解除任何生产门禁，也不能作为第一次 clean independent review。
