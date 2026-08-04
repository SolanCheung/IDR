# IDR V1.3 Trust-Chain Closure Round 11 独立复审报告

审计日期：2026-07-30  
审计对象：`IDR-V1.3-trust-chain-closure-round11-2026-07-30.zip`  
外层 SHA-256：`0a2dc9ef8797ff0f46b932275868006f41c7378f6d6888117c118344f8fc1564`

## 一、总体结论

```text
ROUND11_FIRST_CLEAN_INDEPENDENT_REVIEW = NO

P0 = 2
P1 = 15
P2 = 4

IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

Round 11 是实质性整改，不是形式性补丁。以下能力已经明显加强：

- Production migration 与 Runtime feature、二进制和启动路径已经分离；
- Runtime 启动会拒绝 superuser、DDL、owner、trigger-capable 数据库身份；
- `idr_operation_idempotency_fences` 建立了独立、append-only 的 operation/idempotency 身份；
- Exact Receipt replay 会重新验证当前 CallerAuthentication、命令 Proof Set、数据库时间和当前 Trust Root；
- Receipt→Outcome 与 Outcome→Human Model 引入明确、有限的历史证据窗口；
- Action 失效或过期可以在 Deliver/Dispatch 的语义迁移中形成 `Cancelled` 或 `ReconciliationRequired`；
- 当前 Round 11 包的 allowlist、摘要清单和独立 secret scan 均通过。

但是，Round 10 的数据库信任边界问题并没有真正关闭，只是从“同一高权限身份可以禁用 trigger”变成了“受限 Runtime 身份仍拥有 schema 全表通用 INSERT/UPDATE”。Migration 0006 明确执行：

```sql
GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA ... TO runtime_role
```

随后只收窄了 Execution Reservation、Attempt 和 Fence 的部分权限。对于 `idr_runs`、`idr_contract_records`、`idr_contract_current`、`idr_proof_envelopes`、`idr_audit_events`、`idr_command_receipts`、`idr_audit_checkpoints` 等权威表，Runtime 数据库身份仍保留直接 DML 能力。

这与 Master Spec 的核心不变量冲突：普通 Rust caller 和被攻陷的 application database login 都属于不可信边界；所有生产权威状态必须只能经 Orchestrator。当前数据库凭据本身仍是一条并行权威写入口。

因此，Round 11 不能计为第一次无信任边界 P0 的独立干净复审。

---

## 二、审计范围与独立验证

### 2.1 压缩包完整性

- 外层 ZIP SHA-256 与提交的 `.sha256` 文件一致；
- ZIP 条目：1,074；
- 路径穿越：0；
- 符号链接：0；
- 包内 `SHA256SUMS`：888/888 PASS；
- 包含 `SHA256SUMS` 在内的原始包文件数：889；
- 独立凭据扫描：0 个命中。

### 2.2 独立动态验证

当前独立环境提供 Node.js、npm 和 Python，但没有 Cargo、rustc 与 PostgreSQL：

- TypeScript 离线安装：PASS；
- TypeScript tests：12/12 PASS；
- TypeScript typecheck：PASS；
- Python tests：9/9 PASS；
- Rust/PostgreSQL：未在本环境独立执行。

包内、经 SHA 清单验证的维护者日志声称：

- IDR default Rust workspace：65 项通过，另有 7 个 downstream compile-fail fixture；
- Shadow PostgreSQL 测试：5 项通过；
- Production release-gate profile：17 项通过；
- strict Clippy、rustfmt、migration binary、role attacks、Aegis 1,878 项均通过。

这些属于有价值的维护者执行证据，但不能替代本轮独立 PostgreSQL 动态攻击复现。

---

# 三、Round 10 问题关闭矩阵

| Round 10 项目 | Round 11 独立结论 |
| --- | --- |
| P0-00 泄漏凭据的服务商侧撤销 | **未关闭**。新包干净，但 provider revocation、rotation、last-use 和 billing review 没有外部证据 |
| P0-01 migration/runtime 身份分离 | **结构上完成**。Production Runtime 不再自动 migration，feature 组合被编译阻断 |
| P0-01 Runtime 禁止 DDL/trigger/owner | **完成原始攻击修复**。启动与 ACL 会拒绝这些权限 |
| P0-01 永久 operation/idempotency identity | **正常路径完成**。Fence append-only，Reservation 引用 Fence |
| P0-01 数据库信任边界 | **未关闭**。Runtime 仍拥有所有权威表的通用 INSERT/UPDATE |
| P1-01 Receipt replay 重新认证 | **核心行为关闭**。当前 Proof 和 Trust Root 会重新验证 |
| P1-02/03 历史证据时效 | **基础窗口已实现**，更完整的证据保留与衰减治理仍属 P1 |
| P1-04 无效 Action 状态持久化 | **Action invalidation/expiry 路径完成**；Proof revocation/expiry 路径仍只返回错误，不持久化治理状态 |

---

# 四、已正确实现的部分

## 4.1 Migration 与 Runtime 的编译和运行边界已分离

`production` 与 `migration` feature 互斥，Production Runtime 不再自动调用 migration。独立 `idr-production-migrator` 执行 schema migration。

证据：

- `IDR/crates/idr-store/src/lib.rs:3-15`
- `IDR/crates/idr-runtime/Cargo.toml:7-17`
- `IDR/tools/idr-production-migrator/src/main.rs:1-17`

这关闭了 Round 10 中“同一 Runtime pool 自带 migration/DDL authority”的直接路径。

## 4.2 Production Runtime 身份会拒绝明显高权限

启动检查会拒绝：

- superuser；
- `CREATEROLE`；
- `CREATEDB`；
- replication；
- bypass RLS；
- schema/database CREATE；
- Fence TRIGGER 权限；
- 可切换为任一表 owner 的身份。

证据：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:395-467`

## 4.3 Operation/idempotency Fence 的数据模型方向正确

新表将以下 tuple 永久唯一化：

```text
trust_domain
+ environment_ref
+ tenant_ref
+ operation_ref
+ idempotency_key
```

Fence 的 UPDATE/DELETE 被 trigger 拒绝；Reservation 必须与 Fence 的 Action、operation 和 idempotency identity 精确一致。

证据：

- `IDR/crates/idr-store/migrations/0006_round11_runtime_roles_and_idempotency_fence.sql:15-85`

这比将 exactly-once identity 直接放在可变 Reservation 行中更可靠。

## 4.4 Exact Receipt replay 已增加当前认证复核

在返回旧 Receipt 前，Runtime 会：

1. 比较 trust domain、environment、tenant、run、actor、caller 和 command digest；
2. 重新验证当前 command proof set；
3. 使用当前数据库时间和当前 Trust Root；
4. 然后才反序列化并返回旧 Receipt。

证据：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:1633-1688`

这关闭了 Round 10 中正常、未篡改数据库条件下的 replay authentication bypass。

## 4.5 执行前五类 Admission Proof 会重新验证

`ReserveExecution`、`RecoverPermit`、`DeliverPermit` 和 `StartDispatch` 会重新加载 Action Admission 中绑定的：

- Capability；
- Authority；
- Policy；
- Exact Authorization；
- Action Admission。

并在当前 Trust Root 和数据库时间下重新验签和检查有效期。

证据：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:2052-2140`

## 4.6 历史证据窗口已经显式化

- Execution Receipt→Outcome：最多延后 86,400 秒；
- Outcome→Human Model Candidate：最多延后 604,800 秒；
- invalidated 或 superseded 的记录不能作为历史证据继续消费。

证据：

- `IDR/crates/idr-runtime/src/trust_chain.rs:22-23`
- `IDR/crates/idr-runtime/src/trust_chain.rs:1745-1784`
- `IDR/crates/idr-runtime/src/trust_chain.rs:2388-2410`

## 4.7 包卫生整改继续有效

当前 Round 11 包：

- 使用 allowlist 和 secret scanner；
- 未发现 Round 9 的已知凭据路径；
- 独立扫描为 0 findings。

但这只能证明当前包干净，不能证明过去泄漏的密钥已经失效。

---

# 五、P0 问题

## P0-00：Round 9 泄漏凭据的服务商侧撤销、轮换与影响审查仍未证明

### 证据

Round 11 自己明确声明：

- 没有代码层关闭声明；
- provider revocation、rotation、last-use 和 billing review 仍然需要完成；
- 本地删除与包扫描不能使已泄漏凭据失效。

文件：

- `ROUND11-REAUDIT-RESPONSE.md:21-24`
- `ROUND11-REAUDIT-RESPONSE.md:49-53`
- `VALIDATION-SUMMARY.md:79-88`

### 当前状态

已经完成：

- 当前源目录和审核包不再含已知凭据文件；
- 当前包 secret scan 为 0；
- 打包链改为 allowlist。

仍未证明：

- 原密钥已经由服务商控制面 revoke；
- replacement key 已经完成 rotation；
- 泄漏期间的 last-use、usage、billing 和数据访问已经审查；
- replacement key 的存储边界不再进入源码或审核包。

### 必须修复

提供不含密钥原文的外部事件关闭证据：

```text
provider revocation reference
rotation completion timestamp
old-key last-used review
usage/billing anomaly review
replacement-key storage boundary confirmation
```

在外部事件关闭前，P0-00 保持 OPEN。

---

## P0-01：Production Runtime 数据库角色仍是第二条权威写入口

### 根因

Migration 0006 不是以最小权限方式逐表授权，而是先授予：

```sql
GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA ... TO runtime_role
```

然后只针对：

- `idr_execution_reservations` 的 UPDATE columns；
- `idr_execution_attempts` 的 UPDATE columns；
- `idr_operation_idempotency_fences` 的 UPDATE/DELETE/TRIGGER；

进行收窄。

证据：

- `IDR/crates/idr-store/migrations/0006_round11_runtime_roles_and_idempotency_fence.sql:128-168`

因此 Runtime 身份仍可以直接对以下权威数据执行 INSERT，部分表还能 UPDATE：

```text
idr_runs
idr_run_events
idr_step_snapshots
idr_contract_records
idr_contract_current
idr_contract_dependencies
idr_contract_invalidations
idr_proof_envelopes
idr_proof_consumptions
idr_audit_events
idr_outbox_events
idr_audit_checkpoints
idr_command_receipts
idr_response_send_consumptions
idr_execution_receipts
idr_outcome_records
idr_human_model_*
```

数据库没有启用 RLS，也没有使用仅暴露 `SECURITY DEFINER` transition procedure 的模式。

### 为什么这属于信任边界 P0

Master Spec 明确规定：

- 普通 Rust caller 不可信；
- Store 只接受 Orchestrator Command；
- 所有生产权威状态只能经 Orchestrator。

Threat Model 还明确将“被攻陷的 application database login”列为攻击者。

证据：

- `IDR/docs/trust-chain-closure/THREAT-MODEL.md:1-35`
- `IDR/IDR-V1.3-Trust-Chain-Closure-Master-Spec.md:198-225`
- `IDR/IDR-V1.3-Trust-Chain-Closure-Master-Spec.md:1418-1427`

当前 Runtime DB login 本身可以写权威表，所以 PostgreSQL 凭据成为了 Orchestrator 之外的 authority capability。

### 可复现路径 A：伪造 Command Receipt

`idr_command_receipts` 的 INSERT 约束只要求：

- 引用一个存在的 Run；
- trust domain/environment 与 Run 一致；
- 字段非空/摘要形状合法。

其 trigger 只检查 Run child identity；append-only trigger 只阻止后续 UPDATE/DELETE，没有验证 Receipt JSON 是否确实由某次 transition、event、audit 和 aggregate state 派生。

证据：

- `IDR/crates/idr-store/migrations/0001_idr_v13_trust_chain.sql:346-353`
- `IDR/crates/idr-store/migrations/0004_round9_domain_validity_and_immutability.sql:139-164`
- `IDR/crates/idr-store/migrations/0004_round9_domain_validity_and_immutability.sql:353-361`

Runtime role 可以在一个合法命令进入前直接插入匹配其 command ID、principal 和 command digest 的伪造 Receipt。`handle()` 发现该行后会重新验证合法命令的 Proof，但随后直接反序列化数据库中的 Receipt JSON 并返回；它不会证明该 Receipt 对应一个实际 committed transition，也不会把 Receipt JSON 与 run event、projection 和 audit event 逐字段比较。

证据：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:1633-1688`
- `IDR/crates/idr-runtime/src/trust_chain.rs:1030-1042`

预期结果：调用者获得 `idempotent_replay=true` 的成功 Receipt，但权威语义 transition 可能从未发生。

### 可复现路径 B：永久预占 idempotency tuple

Runtime role 拥有 Fence INSERT 权限。被攻陷的数据库登录可以预先插入任意未来的：

```text
trust_domain/environment/tenant/operation/idempotency_key
```

造成该 operation 永久无法建立合法 Reservation。Fence 的 append-only 设计会把这次未授权预占永久化。

这不会产生重复执行，但会把 exactly-once 安全对象变成永久拒绝服务工具。

### 可复现路径 C：直接构造一致的权威数据集合

由于 Runtime 身份还能直接 INSERT/UPDATE Run、Contract current pointer、Audit、Checkpoint 和其他权威表，攻击者可以计算公开的 canonical digest/hash，并写入一组内部自洽但未经过 Orchestrator Proof/transition 的记录。启动检查能发现部分不一致，但无法区分“由 Orchestrator 写入的正确一致数据”和“攻击者按公开算法伪造的一致数据”，除非每个权威写入都具有 Orchestrator 独占的不可伪造 attestation。

### 现有测试为什么没有覆盖

Round 11 的角色测试覆盖：

- Runtime 不能禁用 trigger；
- 不能修改 Reservation identity columns；
- 不能 DELETE Fence；
- Auditor 只能 SELECT。

证据：

- `IDR/crates/idr-store/tests/postgres_vertical_slice.rs:1459-1579`

没有覆盖：

```text
runtime_role_raw_insert_command_receipt_is_rejected
runtime_role_raw_insert_contract_is_rejected
runtime_role_raw_update_run_projection_is_rejected
runtime_role_preinsert_fence_is_rejected_without_transition_capability
runtime_role_raw_insert_audit_checkpoint_is_rejected
```

### 必须修复

不能只增加更多 trigger。需要改变权限模型：

1. Runtime login 对权威表不应拥有通用 INSERT/UPDATE/DELETE；
2. 表由独立 NOLOGIN owner role 持有；
3. Runtime 只拥有 `EXECUTE` 少量 schema-qualified mutation procedures 的权限；
4. procedure 必须验证不可伪造的 Orchestrator transition attestation，而不是仅信任数据库登录；
5. 或者所有权威行必须携带由 Orchestrator 专用密钥生成的 MAC/signature，并在每次读取和 replay 时验证；
6. Command Receipt 必须绑定：
   - command digest；
   - pre/post aggregate version；
   - event/audit identity；
   - projection digest；
   - Orchestrator attestation；
7. Production 启动必须枚举并验证完整 ACL matrix，拒绝额外 direct grants、继承角色和跨 schema Runtime role membership；
8. 加入上述 raw INSERT/UPDATE 动态攻击测试。

在 Runtime 数据库凭据不能直接创造权威状态之前，数据库信任边界仍未闭合。

---

# 六、P1 问题

## P1-01：Stored Command Receipt 缺少自身不可伪造的完整性证明

即使将来收紧数据库 ACL，Receipt 行仍只有普通 JSON，没有：

- receipt digest；
- Orchestrator signature/MAC；
- event/audit reference；
- pre/post projection digest；
- receipt-to-row consistency trigger。

Replay 只反序列化 JSON。备份恢复错误、migration bug 或更高权限数据库篡改仍可能制造错误 Receipt。

修复：为 Receipt 建立独立、签名的权威记录，Replay 时重新验证其 digest、signature、aggregate/event/audit binding。

## P1-02：Admission Proof 被撤销或过期时，执行聚合不会持久化治理状态

`reverify_bound_action_admission_proofs()` 在 transition 前执行。Capability/Authority/Policy/Authorization 被撤销时，命令直接返回错误，Reservation 状态保持原状。

证据：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:1749-1756`
- `IDR/crates/idr-runtime/src/postgres_authority.rs:2052-2140`

Round 11 测试也显示：Policy key 被撤销后 Reserve 被拒绝，但 aggregate version 保持 9，没有记录 `Cancelled`、`Expired` 或 governance event。

证据：

- `IDR/crates/idr-store/tests/postgres_vertical_slice.rs:770-844`

这不会导致未授权执行，因此不是 P0；但会留下不可恢复或反复失败的聚合状态，并使 `ROUND11-REAUDIT-RESPONSE.md:29` 对“invalid or expired authority 持久化状态”的表述过宽。

修复：对 Reserve/Deliver/StartDispatch 区分 Proof 失效类型，在同一事务中提交显式 `Cancelled`、`Expired` 或 `ReconciliationRequired` governance transition；不能只返回错误。

## P1-03：Trust Root provider 仍由普通调用者注入，且 rotation 未与数据库 epoch 事务化

`PostgresSecurityContextV1::new()` 是公共 API，接收普通 trait object：

- `CurrentTrustRootProviderV1`；
- `AuditCheckpointAnchorBackendV1`；
- expected issuer map。

证据：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:38-75`
- `IDR/crates/idr-runtime/src/postgres_authority.rs:162-198`

Production release gates 当前是 BLOCKED，因此该路径暂不能启动 Production。但在未来解除门禁前，必须将 production security context 的构建改为受保护配置/KMS/control-plane loader，普通 library caller 不能自行指定 Trust Root 或 issuer registry。

Trust Root rotation 还必须与业务事务绑定同一单调 epoch，避免验证时与提交时观察不同根。

## P1-04：External anchor 仍为自声明 durable 的 trait，且存在 commit→publish 窗口

`AuditCheckpointAnchorBackendV1::is_production_durable()` 由实现者自行返回 bool；代码中没有受保护的 WORM/KMS signer 实现。

业务事务先提交 PostgreSQL，随后再调用 anchor publish；进程在两者之间崩溃会留下未锚定尾部。启动时发现 DB ahead 后会把数据库 checkpoint 自动发布到 anchor，这不能替代独立见证者。

证据：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:91-103`
- `IDR/crates/idr-runtime/src/postgres_authority.rs:776-813`
- `IDR/crates/idr-runtime/src/postgres_authority.rs:2144-2180`

修复：使用独立签名 checkpoint、WORM/KMS 后端、outbox/ack state 和明确的 fail-stop publication protocol。

## P1-05：历史 Trust Root 保存与历史 Proof 全量验签未完成

启动完整性主要校验历史 Proof 的结构、引用和当前数据库关系；没有完整保留每个历史 root/key snapshot 并逐 Proof 重放签名验证。

对应开放风险：

- `IDR/docs/trust-chain-closure/REMAINING-RISKS.md:26-27`

## P1-06：Execution Permit 仍不是 provider-facing signed capability

数据库内 Permit/Reservation 绑定已经较强，但没有可由真实 Provider 离线验证的短期签名 capability，也没有明确的 provider receipt acknowledgment。

开放风险：

- `IDR/docs/trust-chain-closure/REMAINING-RISKS.md:24-25`

## P1-07：Response delivery、fan-out Receipt 和 reconciliation 尚未实现

Response Admission 之后仍缺少真实发送证明、分发目标、部分发送、重试和对账闭环。

开放风险：

- `IDR/docs/trust-chain-closure/REMAINING-RISKS.md:25-25`

## P1-08：Production outbox delivery API/worker 边界未完成

Outbox 写入与业务事务绑定，但 claim/acknowledge API 只在：

```rust
#[cfg(all(feature = "shadow-mode", feature = "test-support"))]
```

下公开。

证据：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:556-618`

Production 中尚没有受认证、租户隔离、worker-fenced 的 outbox delivery surface。

## P1-09：Capability、Authority、Policy registry 与 issuer separation 不完整

当前依赖 caller-supplied expected issuer map 和 Trust Root 中的 proof kinds，尚未形成受保护、版本化、可审计的 Registry/control service。

开放风险：

- `IDR/docs/trust-chain-closure/REMAINING-RISKS.md:28-28`

## P1-10：Outcome 语义仍不足以支持复杂真实业务

当前仍缺少完整的：

- conflicting observations；
- partial results；
- attribution review；
- causal uncertainty；
- multi-observer aggregation；
- observation freshness policy。

开放风险：

- `IDR/docs/trust-chain-closure/REMAINING-RISKS.md:29-29`

## P1-11：Human Model 治理尚未完成

缺少完整的：

- sensitive inference classification；
- retention/decay jobs；
- user inspection；
- correction/delete；
- legal hold；
- policy-controlled historical use。

开放风险：

- `IDR/docs/trust-chain-closure/REMAINING-RISKS.md:30-31`

## P1-12：Production 启动没有验证精确数据库 ACL matrix

当前角色检查只验证部分高权限，不验证：

- 每个权威表允许的精确 INSERT/UPDATE/DELETE；
- direct grants；
- 额外 inherited role memberships；
- 其他 schema-specific runtime role；
- PUBLIC 默认权限；
- future table default privileges。

证据：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:395-467`

即使 P0-01 的广泛授权被修复，启动时仍必须 fail closed 地证明 ACL 与签署的 privilege manifest 完全一致。

## P1-13：Migrator 将数据库 URL 放在进程参数中

独立 migrator 通过：

```text
idr-production-migrator <database-url> <schema>
```

接收数据库 URL。

证据：

- `IDR/tools/idr-production-migrator/src/main.rs:5-14`

包含密码的 URL 可能出现在 process list、shell history、CI logs 或 crash diagnostics。应改为受限环境变量、文件描述符、secret mount 或 OS credential provider，并确保日志只输出脱敏连接标识。

## P1-14：Production migration/replay/restore corpus 和长时间故障测试不足

Migration 0006 明确拒绝带 Execution history 的原地升级，要求 replay 到 fresh schema。但尚未提供真实历史数据集的：

- deterministic replay；
- rollback rehearsal；
- backup/restore；
- disaster recovery；
- long-running concurrency/fuzz；
- version N→N+1→restart 验证。

开放风险：

- `ROUND11-REAUDIT-RESPONSE.md:32-38`
- `IDR/docs/trust-chain-closure/REMAINING-RISKS.md:32-33`

## P1-15：缺少 Git provenance，且尚未完成连续两轮干净独立复审

当前主 IDR 目录不是 Git work tree，审核包只能用 SHA 清单固定快照，不能证明：

- commit lineage；
- signed tag；
- code review approvals；
- clean work tree；
- build provenance/SBOM attestation。

两轮连续无信任边界 P0 的独立复审也尚未发生。

证据：

- `VALIDATION-SUMMARY.md:79-95`
- `IDR/docs/trust-chain-closure/REMAINING-RISKS.md:34-36`

---

# 七、P2 问题

## P2-01：历史证据窗口是硬编码常量

86,400 秒和 604,800 秒直接写在 Runtime 中，没有绑定版本化 Policy Contract、业务域或证据类型。

证据：

- `IDR/crates/idr-runtime/src/trust_chain.rs:22-23`

应改为受版本管理的 evidence-use policy，并把 policy revision 纳入 Outcome/Human Model transition digest。

## P2-02：Schema role 名称使用截断 MD5

Production role 名使用：

```text
substr(md5(current_schema()), 1, 16)
```

证据：

- `IDR/crates/idr-store/migrations/0006_round11_runtime_roles_and_idempotency_fence.sql:98-106`

碰撞概率很低，但安全角色名称不应依赖 64-bit 截断 hash。应使用 deployment-supplied、显式 allowlisted role name，或更长的不可冲突标识。

## P2-03：Shadow schemas 共享全局 Runtime/Auditor role

所有 Shadow schema 共享：

```text
idr_shadow_runtime_v1
idr_shadow_auditor_v1
```

证据：

- `IDR/crates/idr-store/migrations/0006_round11_runtime_roles_and_idempotency_fence.sql:98-107`

在并行测试或共享数据库中，一个 role 可能横跨多个 Shadow schema。虽然不是 Production P0，但会降低测试隔离真实性。

## P2-04：文档对权限与失效处理有过度表述

`ROUND11-REAUDIT-RESPONSE.md:40-47` 声称 Runtime 只有所需 table INSERT/SELECT 和极少 column UPDATE，但实际 SQL 是对所有表 `SELECT, INSERT, UPDATE` 后只收窄少数表。

`ROUND11-REAUDIT-RESPONSE.md:29` 又将 Action currentness/expiry 的持久化处理概括为 invalid or expired authority，而 Admission Proof revocation 路径实际只返回错误。

发布文档和测试矩阵必须严格区分已实现条件，否则会使后续审计误判边界已经关闭。

---

# 八、测试覆盖评价

## 8.1 已覆盖的高价值攻击路径

Round 11 已加入或保留：

- production/migration feature 冲突；
- Runtime DDL/trigger/owner 拒绝；
- Execution identity UPDATE 拒绝；
- Fence UPDATE/DELETE 拒绝；
- privileged Reservation tamper 后原 tuple 的重复 Fence INSERT 拒绝；
- revoked key 阻止 Receipt replay；
- revoked Policy 阻止后续 Reservation；
- historical evidence time boundary/invalidation；
- Action invalidation 前后 dispatch 状态分化；
- TypeScript/Python conformance 和 unknown-field checks。

## 8.2 仍必须加入的攻击测试

```text
runtime_role_raw_insert_command_receipt_is_rejected
runtime_role_raw_insert_contract_record_is_rejected
runtime_role_raw_update_contract_current_is_rejected
runtime_role_raw_update_run_projection_is_rejected
runtime_role_raw_insert_audit_event_is_rejected
runtime_role_raw_insert_checkpoint_is_rejected
runtime_role_preinsert_idempotency_fence_without_transition_is_rejected
forged_receipt_cannot_be_returned_by_idempotent_replay
extra_runtime_role_membership_blocks_startup
extra_direct_table_grant_blocks_startup
proof_revocation_persists_cancel_or_reconciliation_transition
migrator_secret_never_appears_in_process_arguments_or_logs
```

至少第一组 raw DML 测试通过前，Round 11 的数据库信任边界不能验收。

---

# 九、推荐实施顺序

## Phase 1：移除 Runtime 的直接权威表 DML

1. 为所有权威表创建独立 owner NOLOGIN role；
2. Runtime login 撤销权威表的通用 INSERT/UPDATE/DELETE；
3. 明确列出仅需要的只读视图和查询权限；
4. 权威 mutation 只能通过受限 procedure 或可验证 transition attestation；
5. 设定 `ALTER DEFAULT PRIVILEGES`，未来新表默认不给 Runtime DML；
6. Production startup 对完整 ACL manifest 做精确比较。

## Phase 2：让所有权威行具有数据库篡改不可伪造性

特别是：

- Command Receipt；
- Run projection；
- Contract record/current pointer；
- Audit event/checkpoint；
- Execution Fence/Reservation/Attempt。

每个对象必须携带 Orchestrator 独占的签名/MAC 或可验证 inclusion proof。数据库一致性不能只依赖攻击者也能计算的普通 SHA-256。

## Phase 3：关闭 Proof revocation 的状态机缺口

对 Admission Proof 在 Reserve/Deliver/Dispatch 时失效的情况，提交显式 governance transition，而非只返回错误。

## Phase 4：完成已知 P1 基础设施

- 受保护 Trust Root service 与 transactional epoch；
- WORM/KMS checkpoint signer；
- provider-facing signed Permit；
- response delivery receipts；
- production outbox worker；
- historical root replay；
- Human Model retention/decay/correction/delete。

## Phase 5：事件关闭和发布治理

- 服务商侧关闭 Round 9 凭据事件；
- 建立 Git repository、signed tags、SBOM/provenance；
- 真实 production corpus migration/restore；
- 完成两轮连续无信任边界 P0 的独立复审。

---

# 十、最终判定

Round 11 已经正确解决了 migration/runtime feature 混用和 Execution Fence 身份释放问题，说明设计正在收敛。

但当前数据库角色模型仍允许被攻陷的 Runtime login 直接写入权威表。只要数据库凭据本身能够创造 Command Receipt、Contract、Audit 或永久 Fence，Orchestrator 就不是唯一控制平面，Rust Trust Root 也没有真正延伸到 PostgreSQL 持久层。

因此：

```text
ROUND11_FIRST_CLEAN_INDEPENDENT_REVIEW = NO
PRODUCTION TRUST ROOT = BLOCKED
```

Round 12 不应新增 Contract。只应完成：

```text
1. Runtime 权威表零通用 DML
2. 精确 ACL manifest 与启动 fail-closed
3. 权威行 Orchestrator attestation
4. forged Receipt/raw authority INSERT 攻击测试
5. Proof revocation 的持久治理迁移
6. provider credential incident 外部关闭
```
