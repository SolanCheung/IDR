# IDR V1.3 Round 9 独立复审报告

审计日期：2026-07-30  
审计对象：`IDR-V1.3-trust-chain-closure-round9-2026-07-30.zip`  
外层 ZIP SHA-256：`17ab632494f5fed8e2ad67392212dd38e8c76b24af995094e95deb87db028544`

## 一、总体结论

Round 9 是一次真实且质量较高的整改。以下两条 Round 8 P0 已得到实质性关闭：

1. `trust_domain + environment_ref` 已进入 Command、Proof、Issuer Key、Trust Root、Authoritative Record、Receipt、Checkpoint 及数据库身份约束，跨 Shadow/Production 或跨 environment 的同签名重放路径已经被封闭。
2. Exact Authorization 的 ID、expiry 与 Action Admission/Execution Reservation 已建立持久绑定；Reserve、Recover、Deliver、StartDispatch 会在 PostgreSQL 事务内使用数据库时间和当前 Trust Root 重新验证该 Exact Authorization。

但是 Round 9 仍然没有达到第一次独立干净复审。当前确认四个 P0，其中一个是需要立即处置的审计包凭据泄漏事件：

```text
ROUND9_FIRST_CLEAN_INDEPENDENT_REVIEW = NO
P0 = 4
P1 = 12
P2 = 4

IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

Round 8 三项 P0 的独立关闭状态：

| Round 8 P0 | Round 9 复审状态 |
| --- | --- |
| 密码学信任域未绑定 | **已关闭** |
| 过期 Exact Authorization 可延迟复用 | **已关闭其原始路径**；但一般权威依赖有效期仍未闭合，形成新的 P0-01 |
| PostgreSQL 权威内容可原地篡改 | **部分关闭**；Contract/Proof/Receipt 等表已加固，但可变 Execution 索引表仍存在未锚定旁路，见 P0-03 |

---

## 二、独立验证结果

### 2.1 包完整性

- ZIP SHA-256：与用户提供的 `.sha256` 一致。
- ZIP 条目：1,051。
- 路径穿越：0。
- 符号链接：0。
- 有效文件：865，其中 864 个文件列入 `SHA256SUMS`。
- 包内清单：**864/864 PASS**。

### 2.2 本环境独立执行

- TypeScript `npm ci --offline`：PASS。
- TypeScript tests：**12/12 PASS**。
- TypeScript typecheck：PASS。
- Python tests：**9/9 PASS**。

当前审计环境没有 `cargo`、`rustc` 和 PostgreSQL，因此包内记录的 Rust 57 项、7 项 compile-fail、PostgreSQL 5 项和 Aegis 1,878 项测试未在本环境重新执行。其原始输出已被包内 SHA 清单固定，可作为维护者执行证据，但不等于本轮独立动态复现。

---

## 三、已正确实现的关键能力

### 3.1 信任域已经进入密码学身份

`ProductionProofClaimsV1`、`ExpectedProductionProofV1`、Issuer Key 和 Trust Root 均包含 `trust_domain` 与 `environment_ref`；`verify_at` 对 Claims、Expected、Key 和 Root 四方身份执行一致性检查。

证据：

- `crates/idr-protocol/src/production.rs:278-379`
- `crates/idr-protocol/src/production.rs:507-577`
- `crates/idr-protocol/src/production.rs:579-727`

### 3.2 完整 Command Envelope 已进入 Proof subject digest

Proof subject 由完整 unsigned envelope 计算，覆盖 command ID、domain、environment、run、expected aggregate version、actor、caller、tenant、scope、purpose、policy、correlation、causation 和 exact command target。

证据：`crates/idr-runtime/src/trust_chain.rs:327-368,525-546`。

### 3.3 权威对象和直接 transition 已封闭

Authoritative record 为 crate-private、Serialize-only；生产 mutation 由具体 PostgreSQL Orchestrator 统一承担。危险 `production + test-support`、`production + shadow-mode` 和 `production + dev-file-store` 组合有 compile-time gate。

证据：

- `crates/idr-runtime/src/lib.rs:1-22`
- `crates/idr-store/src/lib.rs:1-18`
- `crates/idr-runtime/src/trust_chain.rs:121-190`

### 3.4 Decision → Action 精确派生已经实施

Action 的 selected option、operation 和 parameter digest 必须与当前 Decision binding 完全一致。

证据：`crates/idr-runtime/src/trust_chain.rs:1252-1295`。

### 3.5 Exact Authorization 全链有效期原始漏洞已修复

Action Admission 保存 exact authorization proof ID 和 expiry；Admission validity 取 Action 与所有 Proof expiry 的最小值；Reserve/Recover/Deliver/Start 使用 DB time/current root 重新验证 exact authorization。

证据：

- `crates/idr-runtime/src/trust_chain.rs:1312-1390`
- `crates/idr-runtime/src/trust_chain.rs:1392-1506`
- `crates/idr-runtime/src/postgres_authority.rs:1240-1327`
- `crates/idr-runtime/src/postgres_authority.rs:1611-1681`

### 3.6 Human Model materialization 已由 Candidate/Promotion 内部推导

Assertion 的 lifecycle 和 maximum impact 从 Promotion outcome 与 source Candidate 派生，调用方不能再自行升级这些字段。

证据：`crates/idr-runtime/src/trust_chain.rs:1802-1895`。

### 3.7 Contract 记录摘要和专用投影获得较强启动复核

启动过程重新计算 authoritative record digest，比较 current pointer、dependencies、proof ID 关系以及 Receipt、Outcome、Human Model 专用投影。

证据：`crates/idr-runtime/src/postgres_authority.rs:714-1172`。

---

# 四、P0 问题

## P0-00：审计包泄漏疑似真实 API Key，必须立即撤销

### 问题

审计包包含：

```text
Aegis-Life/apps/aegis-web/.aegis/credentials.json
```

该文件中存在一个 `sk-…` 形式、38 字符、未包含 `sample/example/fake/test/redacted/placeholder` 等占位词的 API Key。为避免二次泄漏，本报告不展示密钥原文，仅记录其值的 SHA-256 指纹：

```text
bc64419c2e86860e4a48f06186b6e076a62440d681eab439a13d854964552fcf
```

包根 README 却声明归档排除了真实凭据：

- `README.md:40-43`

`PACKAGE-INVENTORY.txt:34` 明确列出了该 credentials 文件；包内 `SHA256SUMS` 也将它作为正常交付物固定。

Aegis Web 自身 `.gitignore` 已忽略 `/.aegis/`，说明它本应被视为本地运行状态/秘密目录；但 Round 9 打包逻辑没有遵守这一边界。

### 影响

- 凭据可能已随 ZIP、下载副本、废纸篓、聊天附件和审计归档扩散。
- 任何获得包的人都可能调用对应 provider、产生费用、访问关联账户能力或造成追踪困难。
- 即使密钥是低权限或测试密钥，也必须按真实泄漏处理，直到 provider 明确证明其无效。

### 立即处置

1. 立即在对应 provider 控制台撤销并轮换该 Key。
2. 检查使用日志、异常调用、费用和来源 IP。
3. 删除并重新生成 Round 9 ZIP、`.sha256`、包内 `SHA256SUMS` 和所有分发副本。
4. 清理项目目录、Downloads、审计目录、聊天上传副本和废纸篓中的旧包。
5. 将 `/.aegis/`、credential 文件名和 secret scanner 设为打包硬失败条件。
6. 对整个仓库及历史审计包执行 secret scanning；本轮独立扫描除一个第三方类型声明误报外，只确认了这一条 `sk-…` 命中。

### 推荐防复发措施

- 打包使用 allowlist，不再使用“复制后排除”的 denylist。
- CI/打包脚本增加 gitleaks/trufflehog 等扫描，并对 `sk-`、private key、cloud credentials、token/password assignment fail closed。
- package inventory 中出现 `.aegis/`、`.env`、credentials、secrets、runtime state 即失败。
- 真实凭据仅通过系统 keychain/KMS/secret mount 提供，不落入项目树。

---

## P0-01：过期的上游权威记录仍可继续被消费

### 问题

Round 9 给每个 Candidate 和 Authoritative Record 增加了 `valid_from/valid_until`，但一般依赖检查只判断记录“存在、current、未被显式 invalidated”，不判断当前数据库时间是否仍在有效期内。

`require_record` 和 `require_exact_current_record` 均没有 `trusted_now` 参数：

- `crates/idr-runtime/src/trust_chain.rs:2273-2307`

因此以下路径仍合法：

- 过期 Context → 新 Intent；
- 过期 Intent → 新 Decision/Turn；
- 过期 Decision/Turn → 新 Action；
- 过期 Response → 使用新的 ResponsePolicy/ResponseAdmission Proof 发送。

具体消费点：

- `RecordContext/Intent/Decision/Turn/Response/Action`：`trust_chain.rs:1126-1310`
- `ConsumeResponseSend`：`trust_chain.rs:1226-1250`

其中 `issue_record` 只验证**新 Candidate 自身**当前有效，不能证明其来源依赖仍有效：

- `trust_chain.rs:1960-2024`

Master Spec 明确要求权威对象具有有效期，并要求 Action Admission 验证 dependencies current：

- `IDR-V1.3-Trust-Chain-Closure-Master-Spec.md:221-226`
- `IDR-V1.3-Trust-Chain-Closure-Master-Spec.md:704-723`

### 可复现方式

1. 创建 Intent v1，`valid_until=T`。
2. 等待数据库时间超过 `T`，不创建 successor，也不显式 invalidation。
3. 提交一个当前有效的 Decision candidate 和新的 Decision Necessity Proof。
4. `RecordDecision` 只调用 `require_record(Intent)`，因此可创建新 Decision。
5. 继续创建 Turn、Action，并使用新 Authorization/Admission Proof 执行。

更直接的 Response 路径：

1. 创建 Response，等待其 `valid_until` 过期。
2. 为 `ConsumeResponseSend` 生成新的 ResponsePolicy 与 ResponseAdmission Proof。
3. `require_exact_current_record` 不检查时间，因此过期 Response 仍可获得发送许可。

### 影响

有效期只约束对象“创建时”，没有约束其“消费时”。这允许已经失去授权语义或时效性的 Decision、Response 和上游上下文继续产生生产行为。

### 推荐修复

- 将 `require_record` 和 `require_exact_current_record` 改为接受 `trusted_now`，统一要求：

```text
valid_from <= trusted_now < valid_until
AND current
AND not invalidated
```

- 对每个 command 明确定义 `required_current_dependencies()`，在 transition 前统一校验，而不是分散在 match branch 中。
- PostgreSQL transaction 中从 authoritative record 表加载 dependency validity，并进行同事务复核。
- 增加攻击测试：expired Context/Intent/Decision/Turn/Response、expired dependency Action Admission。

---

## P0-02：执行阶段只复核 Exact Authorization，不复核已撤销的 Capability/Authority/Policy Admission Proof

### 问题

`AdmitAction` 创建时要求五类 Proof：

```text
Capability
Authority
Policy
ExactAuthorization
ActionAdmission
```

证据：`crates/idr-runtime/src/trust_chain.rs:632-638`。

Admission record 的 digest 会间接绑定 Proof IDs，但执行阶段的 current Trust Root 复核只重新加载 `exact_authorization_proof_id`：

- `crates/idr-runtime/src/postgres_authority.rs:1611-1681`

Reserve/Recover/Deliver/Start 当前命令只要求新的 `ExecutionPermit` Proof：

- `trust_chain.rs:639-643`
- `postgres_authority.rs:1557-1608`

因此 Capability、Authority、Policy 或 ActionAdmission issuer key 在 AdmitAction 后被撤销时，Admission 仍保持可执行；只要 ExactAuthorization 和新的 ExecutionPermit 仍有效，Reservation/Dispatch 可以继续。

Master Spec 要求 Permit 绑定并执行 Capability、Policy 和 Trust Root current/revocation 语义：

- `IDR-V1.3-Trust-Chain-Closure-Master-Spec.md:255-267`
- `IDR-V1.3-Trust-Chain-Closure-Master-Spec.md:704-741`
- `IDR-V1.3-Trust-Chain-Closure-Master-Spec.md:800-817`

### 可复现方式

1. Trust Root v1 下，用有效 Capability、Authority、Policy、ExactAuthorization、ActionAdmission Proof 创建 Admission。
2. 旋转到 Trust Root v2，撤销 Policy 或 Capability issuer key；ExactAuthorization 与 ExecutionPermit issuer 保持有效。
3. 使用 v2 签发新的 ExecutionPermit Proof，提交 `ReserveExecution`。
4. `reverify_bound_execution_authorization` 只检查 ExactAuthorization；当前命令 Proof 只检查 ExecutionPermit。
5. 被撤销 Policy/Capability 支持的旧 Admission 仍能执行。

### 影响

政策、能力或权限撤销不能即时阻断尚未 dispatch 的 Action。系统当前只实现了 Authorization revocation/currentness，不是完整 Action Admission revocation/currentness。

### 推荐修复

Admission Projection/record 必须持久保存五类 exact Proof refs、issuer/key/root version/expiry。每个 pre-dispatch transition 在同一事务中：

1. 加载 Admission 的全部 Proof envelopes；
2. 使用当前 Trust Root 重新验证签名、issuer、key validity 和 revocation；
3. 重新验证 assertion 与 exact Admission/Action binding；
4. 任何一项失效时，将未 dispatch Reservation 原子取消；dispatch 后转入 reconciliation。

也可以签发一个由 IDR 内部 Admission issuer 生成的短期、不可撤销窗口极小的 `ActionAdmissionAttestation`，但其 revocation epoch 必须由当前 Trust Root 检查，不能仅依赖 expiry。

---

## P0-03：可变 Execution 表可释放 exactly-once 唯一键，且启动完整性检查无法发现

### 问题

`idr_execution_reservations` 依靠以下唯一约束实现 Action-level exactly-once：

```text
UNIQUE (tenant_ref, operation_ref, idempotency_key)
```

证据：`crates/idr-store/migrations/0001_idr_v13_trust_chain.sql:149-171`。

Round 9 的 append-only trigger 和 `REVOKE UPDATE, DELETE` 覆盖 Contract、Proof、Audit、Receipt、Outcome 和 Human Model 表，却**没有覆盖**：

- `idr_execution_reservations`
- `idr_execution_attempts`

证据：`0004_round9_domain_validity_and_immutability.sql:315-382`。

这两个表必须更新状态，但当前没有“只允许合法状态字段按 CAS 更新”的列级 trigger。应用使用与 migration 相同的 pool/数据库身份运行，也没有单独 runtime role 或 RLS：

- `postgres_authority.rs:341-360`

启动 `verify_database_integrity` 对 Reservation 只比较：

- action digest；
- action/admission validity；
- exact authorization kind/expiry。

它不比较：

- operation_ref；
- idempotency_key；
- provider_ref；
- owner_ref；
- state/aggregate_version；
- permit ID；
- dispatch nonce；
- lease；
- attempt state。

证据：`postgres_authority.rs:1077-1092`。

而真实 Run Projection 保存这些完整字段：

- `trust_chain.rs:684-706`

Checkpoint 的 `record_set_root` 只覆盖 Contract records，不覆盖 Execution Reservation/Attempt：

- `postgres_authority.rs:2630-2678`

### 可复现方式

已有活动 Reservation：

```text
tenant = tenant-A
operation = operation:pay
idempotency_key = invoice-100
```

直接执行：

```sql
UPDATE idr_execution_reservations
SET operation_ref = 'operation:tampered',
    idempotency_key = 'tampered-key'
WHERE reservation_id = '<existing-reservation>';
```

不需要禁用任何 Round 9 append-only trigger，因为 Execution 表没有该 trigger。

然后：

1. 重启并运行当前 `verify_database_integrity`，上述字段不会被比较，验证可通过。
2. 为另一个 Run 创建真实 `operation:pay + invoice-100` Reservation，唯一槽已被释放，INSERT 可成功。
3. 原 Run 的权威 Projection 仍保存 `operation:pay + invoice-100`，原 Permit 仍可继续 dispatch。
4. 同一真实外部副作用现在拥有两个活动 Reservation。

### 影响

数据库凭据泄漏、SQL injection、错误运维脚本或表 owner 误操作可以在不修改 Audit chain/Contract record-set root 的情况下破坏 exactly-once，并允许重复外部副作用。

这也是 Round 8 “权威内容不可原地篡改”只被部分关闭的原因。

### 推荐修复

- 将 immutable Reservation identity 与 mutable lifecycle 分表：

```text
execution_reservation_identity  -- append-only
execution_attempt_identity      -- append-only
execution_state_transitions     -- append-only CAS events
```

- 或至少增加 `BEFORE UPDATE` trigger，只允许：
  - state 按合法状态图迁移；
  - aggregate_version 严格 +1；
  - timestamp 单调更新；
  - 禁止更改 tenant/action/admission/operation/idempotency/provider/owner/permit/nonce/validity。
- 运行时使用受限非 owner DB role；migration role 与 application role 分离。
- startup integrity 将 Execution identity/attempt 与 Run Projection 全字段比较。
- Checkpoint 加入 canonical execution-state root，或把 execution state transition 纳入完整 event replay root。
- 增加 privileged SQL tamper 与 duplicate side-effect 回归测试。

---

# 五、P1 问题

## P1-01：Trust Root rotation 与数据库事务没有串行化

`RotatingTrustRootProviderV1` 是进程内 `RwLock`；事务中只读取一次 snapshot，之后 root 可在 commit 前并发旋转。

证据：

- `postgres_authority.rs:33-70`
- `postgres_authority.rs:1240-1327`

应将 root version/revocation epoch 持久化到受保护控制面，并在 transaction commit 前通过 CAS/lease 再确认。

## P1-02：External Anchor 发布仍在数据库 commit 之后

事务先提交，再调用 anchor；启动时若 anchor 缺失或落后，会把数据库 checkpoint 重新发布到 anchor。

证据：

- `postgres_authority.rs:647-712`
- `postgres_authority.rs:1553-1554`
- `postgres_authority.rs:1689-1722`

该逻辑适合恢复 post-commit publication failure，但在 anchor 丢失/错误配置时会把数据库重新当作信任源。需要 WORM/KMS signer、pending/committed 双阶段 checkpoint 或独立 witness quorum。

## P1-03：Anchor durability 由公共实现自我声明

`AuditCheckpointAnchorBackendV1::is_production_durable()` 是公共 trait 方法；任何下游实现都可返回 true。Production constructor 只依赖该布尔值。

证据：`postgres_authority.rs:85-97,299-315`。

Production 应只接受 sealed/allowlisted implementation 或由部署 attestation 验证的 anchor capability。

## P1-04：Trust Root 与 issuer registry 由调用者注入，缺少受保护控制服务和职责分离

`PostgresSecurityContextV1::new` 接受任意 Trust Root provider 和 `expected_issuers` map。

证据：`postgres_authority.rs:155-190`。

当前 release blocked 是正确的；解除前必须由受保护 control plane 提供 immutable bootstrap identity、KMS references 和 separation-of-duties policy。

## P1-05：内部 Permit 尚不是 provider-facing signed capability

ExecutionPermit 是进入 Orchestrator command 的 Proof，但没有一个 Executor/Provider 可离线验证、短期、一次性、签名的 capability token。该项也已被包内 `REMAINING-RISKS.md` 承认。

## P1-06：Response 只有 send consumption，没有真实 delivery/fan-out receipt

系统可证明允许发送及 nonce 被消费，不能证明目标渠道实际接受、部分 fan-out、重试或最终交付。

## P1-07：启动时不重新验证历史 Proof 的签名和当时 Trust Root

启动 integrity scan 只反序列化 envelope 并比较 Claims 与表列；没有用保存的 root snapshot/key 重新执行签名验证。

证据：`postgres_authority.rs:913-973`。

历史 Proof evidence 在 privileged DB tamper 后不能仅靠字段一致性证明真实性。需要 root history、proof digest root 和 deterministic replay。

## P1-08：Round 9 migration 无法直接升级有数据的 Round 8 数据库

Migration 先给已有 Contract/Execution 行写入 `epoch` 默认值，然后立即添加：

```text
valid_from < valid_until
action/admission/authorization_valid_until > created_at
```

证据：`0004_round9_domain_validity_and_immutability.sql:1-86`。

非空数据库会在添加约束时失败。注释称旧记录需要独立 replay，但没有提供 migration/backfill/replay 工具或兼容性测试。

## P1-09：Migration role 与 runtime role 未分离

同一数据库连接既运行 `MIGRATOR.run` 又执行 Runtime 命令。`REVOKE ... FROM PUBLIC` 不限制 table owner。

证据：

- `postgres_authority.rs:341-360`
- `0004_round9_domain_validity_and_immutability.sql:368-382`

生产应提供 owner/migrator、runtime writer、read model、outbox worker 等最小权限角色。

## P1-10：Outcome conflict、attribution 与 observation aggregation 仍是单记录最小模型

缺少多观察者冲突、归因审查、迟到观察、环境变化和不确定 Outcome 的完整调度与治理。包内 Remaining Risks 已承认。

## P1-11：Human Model 敏感推断、保留、衰减、删除和查询控制不完整

Round 9 修复了 assertion materialization，但还没有敏感属性分类、retention job、decay、用户删除/纠正体验和 production read authorization。

## P1-12：关键攻击矩阵仍缺少动态测试

当前测试未覆盖本报告三个 P0 的核心路径：

- expired Intent/Decision/Response consumption；
- Capability/Authority/Policy key 在 Admission 后、dispatch 前撤销；
- Execution reservation identity SQL mutation 后重复 Reservation。

此外仍缺长时间 fuzz、crash/chaos、真实 multi-process Postgres rotation/revocation evidence。

---

# 六、P2 问题

## P2-01：时间单位与 Master Spec 漂移

Master Spec 要求整数 Unix milliseconds：`Master-Spec.md:391-396`。实现和公开字段使用 epoch seconds，例如：

- `postgres_authority.rs:239`
- `postgres_authority.rs:1240-1247`

当前三语言内部可能一致，但规范与实现语义不一致，应在 V1 wire freeze 前统一。

## P2-02：没有 Git provenance

包内 `GIT-STATE.txt` 说明主 IDR 根目录没有 `.git`。SHA 清单能固定快照，不能替代作者历史、review commits、signed tag 和可验证差异链。

## P2-03：Round 9 文档对 PostgreSQL 不可变性的描述过宽

`ROUND9-REAUDIT-RESPONSE.md` 声称 startup cross-checks execution 并解决 mutable authoritative content，但 Execution Reservation/Attempt identity 并未进入 append-only 和完整 replay。应将结论改为“Contract/evidence tables closed；operational execution identity pending”。

## P2-04：Checkpoint 表的可变字段缺少列级保护

`idr_audit_checkpoints` 需要更新 `published_at`，但没有 trigger 限制 UPDATE 只能修改这一列。当前 external latest anchor 可发现多数 root 修改，仍建议增加 column-specific immutable trigger，并验证完整 checkpoint history，而非仅每 tenant 最新行。

---

# 七、测试覆盖评价

Round 9 测试数量和跨语言 conformance 良好，尤其包括：

- foreign domain/environment signature rejection；
- exact authorization transaction-time revocation；
- contract trigger bypass tamper detection；
- generated drift；
- compile-time feature gates。

但测试数量不能替代状态空间覆盖。本轮三个 P0 均位于“已经加固的子系统之间的接缝”：

```text
record validity ↔ dependency consumption
admission proofs ↔ execution-time revocation
run projection ↔ mutable execution indexes
```

Round 10 应暂停新增类型，只为这些接缝建立 executable attack regressions。

---

# 八、下一阶段实施顺序

## Phase 0：先完成凭据泄漏事件响应

- 撤销/轮换泄漏 Key；
- 审计调用日志和费用；
- 重新生成无凭据的审计包；
- 添加 secret-scan 与 allowlist packaging gate。

## Phase 1：统一 Current Record 消费门禁

- 引入 `CurrentRecordAtV1`/`require_current_record_at(trusted_now)`。
- 所有 command dependencies 通过统一表驱动校验。
- expired record 自动视为 non-current，必要时生成明确 expiry event/invalidation projection。

## Phase 2：Admission Proof Bundle 的执行期复核

- Admission 保存全部 exact proof refs。
- Reserve/Recover/Deliver/Start 在当前 Trust Root 下复核 Capability、Authority、Policy、ExactAuthorization 和 Admission。
- revocation 后按 dispatch 边界原子 Cancel 或 Reconciliation。

## Phase 3：Execution identity append-only 化

- identity/state 分表或列级 transition trigger。
- application runtime role 不能修改 immutable identity。
- startup 全字段比较 DB execution rows 与 Run Projection。
- execution root 纳入 checkpoint/external anchor。

## Phase 4：只添加针对性攻击测试

必须至少增加：

```text
expired_context_rejects_intent
expired_intent_rejects_decision
expired_decision_rejects_action
expired_response_cannot_send
revoked_policy_after_admission_blocks_reserve
revoked_capability_before_dispatch_cancels_permit
execution_identity_update_is_rejected
execution_index_tamper_detected_on_startup
operation_idempotency_slot_cannot_be_released_by_sql_update
```

## Phase 5：完成 P1 基础设施后再申请连续干净复审

- protected Trust Root/issuer control；
- WORM/KMS checkpoint signer；
- provider-facing signed Permit；
- delivery receipt；
- production migration/replay；
- production DB role model。

---

# 九、最终判定

Round 9 不应被描述为“修完三个 P0 后只剩部署基础设施”，且当前审计包本身必须先按凭据泄漏事件处置。更准确的结论是：

```text
Cryptographic trust-domain binding = CLOSED
Exact Authorization delayed-expiry path = CLOSED
Contract-record append-only/digest replay = SUBSTANTIALLY CLOSED
Audit package credential hygiene = FAILED (P0)
General dependency validity = OPEN (P0)
Full Admission revocation at execution = OPEN (P0)
Execution exactly-once storage integrity = OPEN (P0)
```

因此本轮不能计为第一次干净独立复审，所有生产发布门必须继续保持关闭。
