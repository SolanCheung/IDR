# IDR V1.3 Trust-Chain Closure Round 6 独立复审报告

审计日期：2026-07-29  
审计对象：`IDR-V1.3-trust-chain-closure-round6-2026-07-29.zip`  
ZIP SHA-256：`c244371877c55392d47be6a9354b1f2ce7af2ea491e1f8ea8418a9e3e3d9f4bc`  
规范基线：`IDR-V1.3-Trust-Chain-Closure-Master-Spec.md`  
前置审计：`IDR-V1.3-round5-reaudit-report-2026-07-29.md`

## 1. 总体结论

Round 6 不是一次表面修补。它已经完成了几项重要的结构升级：

- 引入 `CandidateSubmissionV1`、Ed25519 Proof 和 opaque `VerifiedProductionProofV1`；
- `AuthoritativeRecordV1` 改为私有字段、仅可序列化；
- 建立 PostgreSQL `SERIALIZABLE` 事务路径、DB time、proof replay、audit/outbox；
- 新增 Action Admission、Execution Reservation、Permit、Receipt、Outcome 和 Human Model 专用表；
- 修复 Round 5 的 Permit 过期恢复、Action 失效前取消和并发 attempt-1 等问题；
- 建立 Rust/TypeScript/Python canonical encoding 与 golden vector；
- Aegis production adapter 继续 compile-blocked。

但是，Round 6 **不是第一轮“零信任边界 P0”复审**。本次独立源码审查确认：

```text
P0 = 14
P1 = 10
P2 = 6
```

因此 Codex 保持以下门禁关闭是正确的：

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

更重要的是，包内 `ROUND5-CLOSURE-MATRIX.md` 将“公共权威颁发”“Orchestrator 唯一控制平面”“Human Model Gate bypass”等标记为 Closed，这些结论与实际代码不一致。Round 6 的主要缺口已经从“缺少对象”转移为：

> **安全类型存在，但公开 API、投影存储和跨对象语义没有把这些类型串成唯一不可绕过的生产链路。**

---

## 2. 独立验证结果

### 2.1 包完整性

- ZIP 路径穿越：0
- 符号链接：0
- 包内 `SHA256SUMS`：811/811 PASS
- ZIP SHA-256 与提交值一致

### 2.2 本审计环境独立执行

- TypeScript `npm ci --offline`：PASS
- TypeScript tests：12/12 PASS
- TypeScript typecheck：PASS
- Python tests：9/9 PASS

独立输出：`idr-v13-round6-independent-test-output.txt`

### 2.3 未能独立执行的范围

当前审计容器没有 `cargo`、`rustc` 和 PostgreSQL，因此无法独立重跑：

- Rust workspace 58 项；
- trybuild 5 项；
- PostgreSQL 测试；
- Clippy/rustfmt；
- Aegis Rust 1,878 项。

包内原始输出完整地记录了这些测试通过，但它仍属于交付方提供的执行证据，不等同于本轮独立动态复现。本报告的 P0 结论来自具体公开 API、分支条件、SQL 约束和可构造调用路径，不依赖 Rust 测试是否通过。

---

## 3. 原始设计与实现对应关系

| 设计能力 | Round 6 状态 | 结论 |
| --- | --- | --- |
| Rust 是生产信任根 | opaque Proof/Record 已实现 | **部分实现**；普通 Rust 调用者仍可通过自定义 Repository 和 raw PgPool 绕过 |
| TypeScript 只负责产品边界 | 解析、canonical golden、无 Rust guard 副本 | **基本正确** |
| Python 只负责离线评估 | 无生产写入口 | **正确** |
| 唯一 Orchestrator 写入口 | 有 `IdrOrchestratorV1` | **未闭合**；Repository trait、transition function、PgPool 均公开 |
| Proof transaction-time verification | DB time + Trust Root + nonce consumption | **大体正确**；受公开旁路和未保护 Trust Root 限制 |
| Context/Intent/Decision/Turn 强制链 | 按顺序要求 current records | **结构存在**；subject/turn lineage、state 和 successor invalidation 不完整 |
| Action Derivation | Action 与 Decision 都有 selected option 字段 | **未实现**；二者未比较 |
| Action Admission 是唯一 Reservation 前置 | PostgreSQL FK/trigger 存在 | **正常 Postgres 路径成立**；自定义 Repository 可绕过 |
| 唯一 Execution Reservation | 有 action 和 idempotency unique constraints | **语义错误**；operation key 使用 Action record identity，不是真实 operation |
| Receipt 精确绑定 | Permit/provider/nonce/attempt 已绑定 | **部分正确**；request digest 不与 Action/parameters 比较 |
| Outcome 真实观察 | 要求 OutcomeObservation proof | **部分正确**；结果与 Action/Decision/目标的语义关系不足 |
| Human Model 三对象链 | Candidate/Promotion/Assertion 分表 | **结构存在但可内容替换** |
| PostgreSQL 原子权威 Store | 主事务同时写多表 | **部分正确**；raw pool、投影篡改、command replay 冲突仍存在 |
| Audit/anchor | hash chain + external interface | **部分正确**；hash 未覆盖全部权威元数据/投影，生产 anchor 不存在 |
| Aegis Life 仅参考宿主 | shadow candidate-only，production compile-blocked | **正确** |

---

## 4. 已正确实现的部分

### 4.1 受约束 wire 类型和跨语言 canonical encoding

`crates/idr-protocol/src/production.rs:142-246` 将 Candidate 明确标为不可信输入，并限制 payload 大小和 JSON 复杂度。Proof claims 绑定 issuer、subject digest、audience、tenant、scope、purpose、policy、时间、nonce 和 principal digest。  
JCS profile 拒绝浮点和 JS 非安全整数，TypeScript/Python golden vector 独立测试通过。

### 4.2 Proof 的签名、类型、租户、时间和撤销检查

`production.rs:573-620` 对 proof kind、issuer、subject、digest、audience、tenant、scope、purpose、policy、key permission、有效期和撤销时间执行 Ed25519 校验。  
`postgres.rs:521-583,726-775` 在数据库事务内获取 DB time、加载 Trust Root、校验精确 proof set，并持久化 proof ID/nonce consumption。

### 4.3 Authoritative Record 的直接构造受到限制

`trust_chain.rs:95-168` 的 `AuthoritativeRecordV1` 没有公共构造器和 Deserialize。trybuild 覆盖了直接构造 record、ref、verified proof 和 authority token 的常规路径。

### 4.4 PostgreSQL 正常路径具有较强原子性

`postgres.rs:492-724` 在一个 SERIALIZABLE transaction 内完成：

- aggregate projection；
- step snapshot；
- contract/current/dependencies；
- proof consumption；
- execution/outcome/Human Model specialized tables；
- run event、audit event、outbox、command receipt、checkpoint。

### 4.5 Execution Round 5 问题得到实质修复

`trust_chain.rs:1025-1245` 已建立 PermitIssued、PermitDelivered、DispatchStarted、ReconciliationRequired、Succeeded/Failed/Expired 等状态。  
Round 6 测试覆盖了：

- 两个 Authorization 不能并发创建 attempt-1；
- Action 失效后未 dispatch Permit 被取消；
- lost Permit 过期后持久化并允许下一 attempt；
- Receipt 必须对应 dispatched Permit；
- failed attempt 可以连续 retry。

### 4.6 Aegis 方向正确

Aegis adapter 只创建不可信 host authority candidate，不依赖 `idr-store`，production feature 明确 compile-blocked。IDR 核心没有反向依赖 Aegis。

---

# 5. P0 问题

## P0-01：公开 Repository trait 可“借用”Orchestrator Authority 颁发权威记录

### 代码证据

- `crates/idr-runtime/src/trust_chain.rs:731-751`：`OrchestratorAuthorityV1` 私有字段，但 `IdrTransactionalRepositoryV1` 是公共、下游可实现的 trait；`transact()` 会收到 authority 引用。
- `trust_chain.rs:753-775`：`IdrOrchestratorV1<R>::new(repository)` 是公共泛型构造器。
- `trust_chain.rs:785-810`：`evaluate_authoritative_transition_v1` 是公共函数，只要求调用方持有 authority。
- `trust_chain.rs:501-546`：`RecordResponse`、`RecordAction`、`ReserveExecution`、`PromoteHumanModelAssertion` 等命令的 domain proof set 为空。
- `trust_chain.rs:609-645`：`IdrRunProjectionV1` 字段全部公开且可 Deserialize。
- `crates/idr-runtime/tests/ui/store_authority.rs:1-5` 只证明不能直接写 `_private` 字段，没有测试恶意 Repository 实现。

### 可复现方式

下游 crate：

1. 实现 `IdrTransactionalRepositoryV1`；
2. 创建 `IdrOrchestratorV1::new(MaliciousRepository)`；
3. 在 `transact(authority, ...)` 内忽略外部命令；
4. Deserialize 一份伪造的 Running projection，植入 synthetic Decision/Turn refs；
5. 构造 `RecordAction` command；
6. 调用公开的 `evaluate_authoritative_transition_v1(authority, projection, command, now, &[])`；
7. 读取并序列化返回的 `AuthoritativeRecordV1`。

由于纯 transition 只比较 domain proof set，`RecordAction` 的空 proof set可以通过。该路径完全绕开 PostgreSQL、Trust Root、nonce consumption、audit、outbox 和 DB time。

### 修复方案

- 将 `IdrTransactionalRepositoryV1`、`OrchestratorAuthorityV1`、`evaluate_authoritative_transition_v1` 移入私有/sealed 模块；
- 生产构建只公开一个非泛型 `PostgresIdrOrchestratorV1::connect(...)` factory；
- 测试 Repository 放入 crate-private test support；
- 增加 downstream compile-fail：禁止实现 production Repository trait、禁止调用 transition；
- 即使内部 transition 也必须消费一个 crate-private `VerifiedCommandContextV1`，而不是空 proof slice。

---

## P0-02：`PostgresIdrStoreV1::pool()` 暴露完整可写数据库旁路

### 代码证据

`crates/idr-store/src/postgres.rs:283-285`：

```rust
pub fn pool(&self) -> &PgPool
```

`connect()` 使用同一个能够迁移并写入所有权威表的数据库身份。测试代码已经用 `store.pool()` 直接执行 SQL。

### 可复现方式

普通 Rust 调用者：

```rust
sqlx::query(
    "UPDATE idr_runs SET projection = $1 WHERE run_id = $2"
)
.bind(attacker_projection)
.bind(run_id)
.execute(store.pool())
.await?;
```

还可以直接：

- 插入/删除 contract invalidation；
- 修改 execution attempts；
- 读取 raw Human Model assertion；
- 标记 outbox delivered；
- 修改 audit metadata。

这些操作不经过 Orchestrator、Proof、CAS、audit hash 或 outbox。

### 修复方案

- 删除公共 `pool()`；最多提供 `pub(crate)`；
- 测试使用独立 test-only accessor；
- production 数据库用户使用最小权限和 stored procedures；
- 应用角色不得直接 DML 权威表；
- 为产品读模型建立独立只读 DB role/view。

---

## P0-03：当前 Projection 是可修改的裸 JSON，且不受 audit root 保护

### 代码证据

- `0001_idr_v13_trust_chain.sql:5-14`：`idr_runs.projection jsonb NOT NULL`。
- `postgres.rs:530-547,800-815`：直接从 JSON Deserialize 为 `IdrRunProjectionV1`，没有 projection digest、signature 或 replay verification。
- `postgres.rs:594-602`：audit hash 只覆盖 event type、command ID、run ID、aggregate version、command payload 和 proof IDs。
- `postgres.rs:636-656`：actor、caller、correlation、causation、contract refs、policy revision 是表列，但不在被 hash 的 `audit_payload` 中。
- external anchor 只锚定 audit event hash，不锚定 current projection。

### 可复现方式

1. 执行一个合法 command 并获得外部 anchor；
2. 直接修改 `idr_runs.projection.records`、`state`、`execution.owner_ref` 或 `dispatch_nonce`；
3. 不修改任何 audit row；
4. 提交 expected aggregate version 匹配的新 command。

Store 会直接信任篡改后的 projection；外部 anchor 不会检测。

同样，修改 audit 表中 actor/caller/policy/contract refs 也不会改变 event hash。

### 修复方案

- Projection 不作为独立权威事实，只作为 event replay cache；
- 每次加载验证 `projection_digest`，digest 必须纳入 audit hash/checkpoint；
- 定期或每次命令从 canonical run events + authoritative records 重建并比对；
- audit hash 必须覆盖完整 event envelope、actor/caller/context/policy/contract refs、projection root；
- DB role 禁止直接 UPDATE projection；
- 增加 projection mutation 和 audit-column mutation 攻击测试。

---

## P0-04：同一 Run 的 subject/turn lineage 可以被切换

### 代码证据

- `trust_chain.rs:353-364` 校验 candidate 只比较 run、tenant、scope、purpose、policy 和 kind，**不比较 subject_ref 与 turn_id**。
- `trust_chain.rs:825-840` StartRun 把初始 subject/turn 写入 projection。
- `trust_chain.rs:1420-1471` 后续 Authoritative Record 直接复制 candidate 的 subject/turn。
- SQL revision trigger `0001...sql:364-384` 只检查 revision 连续，不检查 run/tenant/subject/turn lineage。

### 可复现方式

1. StartRun：`subject:user-A`、`turn:T1`；
2. RecordContext：同 run/tenant/scope/purpose/policy，但 candidate 为 `subject:user-B`、`turn:T2`；
3. Context issuer 对该 candidate 签名；
4. command 被接受，Contract record 属于 user-B，而 aggregate projection 仍记录 user-A/T1。

后续可制造跨用户 Intent、Outcome 或 Human Model。

### 修复方案

所有非 StartRun command 在 transaction 内必须强制：

```text
candidate.subject_ref == projection.subject_ref
candidate.turn_id == projection.turn_id
candidate.tenant_ref == projection.tenant_ref
```

若支持多 turn，应建立显式 Turn aggregate/StartTurn command，而不是静默切换。

---

## P0-05：Action 没有证明来自 Decision selected option

### 代码证据

- `trust_chain.rs:1715-1722`：Decision validator 只检查 `necessity_outcome` 和 selected option 是合法 Reference。
- `trust_chain.rs:1725-1733`：Action validator 只检查 operation、selected option 和 parameter digest 的格式。
- `trust_chain.rs:955-966`：RecordAction 只要求存在 Decision 和 Turn，未比较 Decision/Action 的 selected option。
- `postgres.rs:1021-1027`：只创建结构 dependency，不检查语义映射。
- `REMAINING-RISKS.md:18-20` 承认没有 Action Derivation record，但称当前路径有 selected-option binding；代码实际上没有该 binding。

### 可复现方式

- Decision：`selected_option_ref = option:retain-supplier`
- Action：`selected_option_ref = option:delete-supplier`
- Action operation：`operation:delete`
- parameter digest 合法
- CallerAuthentication proof 合法

RecordAction 会被权威签发，之后 Action Admission proof 只绑定该 Action digest，不再发现它与 Decision 不一致。

### 修复方案

- 新增内部 `ActionDerivationRecordV1`；
- 在同一 transaction 内加载 Decision payload；
- 强制 Decision selected option == Action selected option；
- operation/parameter digest 必须由 option payload 的确定性 mapper 生成；
- Action candidate 不允许调用者自行提供这些权威字段；
- 增加 Master Spec 21.4 的 mismatch 动态测试。

---

## P0-06：上游 Contract 换版不会自动递归失效下游

### 代码证据

- `postgres.rs:1055-1075` successor 仅新增 predecessor dependency。
- `postgres.rs:560-563,1096-1137` 只有显式 `InvalidateRecord` command 才计算 recursive closure。
- `persist_authoritative_record()` 更新 current pointer，但不触发依赖旧 revision 的 Decision/Turn/Action 失效。
- `AdmitAction` 只检查 Action 自身仍是 current，不检查 Action 的 dependency freshness。

### 可复现方式

1. Intent v1 → Decision v1 → Turn v1 → Action v1；
2. 记录 Intent v2；
3. Intent current pointer 更新为 v2；
4. Decision v1、Turn v1、Action v1 仍在 projection 中且未 invalidated；
5. 使用 Action v1 执行 AdmitAction/ReserveExecution。

### 修复方案

任何 successor 提交时必须在同一事务中：

1. 找到 predecessor；
2. 计算 reverse dependency closure；
3. 标记所有依赖旧精确 revision 的下游失效；
4. 取消未 dispatch execution，dispatch 后进入 reconciliation；
5. 清理 current admission/response bindings；
6. 生成重算步骤。

---

## P0-07：Run 终态没有成为所有 command 的全局门禁

### 代码证据

- `trust_chain.rs:827` 是整个文件唯一实际调用 `require_state()` 的地方。
- RecordContext、RecordIntent、RecordAction、AdmitAction、ReserveExecution、DeliverPermit、StartDispatch 等没有检查 run state。
- `trust_chain.rs:1380-1388` CancelRun 只把状态改为 Cancelled。
- `postgres.rs:907-928` 每个 command 的 step snapshot 都无条件写为 `'completed'`。
- WaitingInput、WaitingAuthorization、WaitingDependency、Succeeded、Failed 等状态在生产 transition 中基本没有被驱动。

### 可复现方式

1. 完成 Action Admission；
2. 提交 CancelRun；
3. 以新的 CallerAuthentication proof 提交 ReserveExecution；
4. 再 DeliverPermit/StartDispatch。

由于这些命令不检查 Cancelled，执行可以继续。

### 修复方案

- 每个 command 明确允许的 RunState/StepState；
- 在 match 前执行 centralized transition matrix；
- Cancelled/Rejected/Succeeded/Failed/TimedOut/Invalidated 默认拒绝所有写命令，只有限定 recovery/read 命令例外；
- step snapshot 根据真实状态写 pending/running/waiting/completed/failed；
- production 应使用真正的 Orchestration aggregate，而不是 legacy-shadow 模型。

---

## P0-08：Execution lifecycle 命令没有 owner/provider 权限绑定

### 代码证据

- `trust_chain.rs:501-546`：ReserveExecution、RecoverPermit、DeliverPermit、StartDispatch、ExpireExecutionLease、CancelRun、PromoteHumanModelAssertion 的 domain proof set 为空；
- `IdrCommandEnvelopeV1::required_proof_kinds()` 在空集合时只补一个通用 `CallerAuthentication`（451-456）。
- `trust_chain.rs:1025-1145` 只比较命令中的 provider/owner 与 projection，未检查 `command.actor_ref/caller_ref` 等于 owner/provider，也不要求专用 lifecycle proof。
- `IdrOrchestratorV1::inspect(run_id)` 无认证，可暴露 reservation/permit/nonce。

### 可复现方式

取得合法 CallerAuthentication 的另一个主体，在知道 reservation ID/permit ID 后，可调用：

- RecoverPermit；
- DeliverPermit；
- StartDispatch；
- ExpireExecutionLease。

没有 provider identity proof 或 owner delegation proof。

### 修复方案

- Reserve 必须由 Admission owner proof 驱动；
- Deliver/StartDispatch 必须由 exact provider/dispatcher proof 驱动；
- Recover/Expire 必须由 owner/lease-manager proof 驱动；
- principal digest 中加入 reservation/permit/provider/owner identity；
- transition 中明确比较 actor/caller；
- inspect API 必须经过 authenticated read context。

---

## P0-09：Exactly-once 唯一键使用 synthetic record identity，不是真实 operation

### 代码证据

- Action payload 有真实 `operation_ref`：`trust_chain.rs:1725-1733`。
- `ExecutionProjectionV1` 不保存 operation_ref。
- `postgres.rs:1241-1267` 持久化 reservation 时写入：

```text
operation_ref = "action-record:{record_id}:{revision}"
```

- SQL 唯一约束为 `(tenant_ref, operation_ref, idempotency_key)`：`0001...sql:149-170`。

### 可复现方式

两个 Action：

- 都代表 `operation:transfer-funds`；
- idempotency key 相同；
- Action record ID 或 revision 不同。

数据库中的 synthetic operation_ref 不同，因此两个 Reservation 均可插入并 dispatch。

### 修复方案

- Action authoritative payload 必须持久化 canonical operation_ref；
- Reservation 从 Action record 中读取 operation_ref，不接受命令自报；
- 唯一索引使用真实 `(tenant, operation_ref, idempotency_key)`；
- 定义 completed/failed/retry 后的 idempotency 生命周期；
- 加跨 run、跨 Action revision、并发重复副作用测试。

---

## P0-10：Receipt 的 request digest 没有和 Action 参数绑定

### 代码证据

- `trust_chain.rs:1872-1905` 只检查 `request_digest` 是 64 hex；
- 它只比较 Permit、provider、nonce、attempt 和 invalidation flag；
- Execution projection/table 没有保存 Action parameter digest 或 canonical request digest；
- SQL Receipt trigger `0001...sql:484-510` 也不比较 request digest 与 Action。

### 可复现方式

Provider 对一个 Receipt candidate 签名，Receipt 中：

```text
request_digest = digest(other_request)
```

只要格式合法、Permit/provider/nonce/attempt 正确，Store 接受。Provider Proof 证明“Provider 签了这个错误值”，不证明它等于获批 Action。

### 修复方案

- Reservation/Permit 固化 Action record digest、operation、parameter digest、request digest；
- Provider-facing Permit 对这些字段签名；
- Receipt candidate 必须精确回显这些字段；
- runtime 和 SQL trigger 双重检查。

---

## P0-11：Human Model Assertion 可以替换已获批 Candidate 的内容

### 代码证据

- Promotion decision 只绑定 source candidate digest：`trust_chain.rs:1919-1942`。
- PromoteHumanModelAssertion 只要求 assertion payload 的 `source_candidate_digest` 相同：`1944-1966`。
- Assertion 的 predicate、value_digest、scope、evidence、allowed purposes 重新由调用者提供，并未与 source candidate 比较。
- `PromoteHumanModelAssertion` 不要求 HumanModelPromotion proof，只补通用 CallerAuthentication。
- SQL assertion 表只 FK 到 promotion decision，不检查内容一致性。

### 可复现方式

1. Candidate：`predicate=ui.theme`、value=`dark`；
2. 获得合法 promotion decision；
3. Assertion 使用同一 `source_candidate_digest`，但改为：
   - `predicate=authorization.confirmation_threshold`
   - 新 value digest
   - 新 scope/allowed purposes；
4. lifecycle 设为 `user_confirmed`。

当前校验通过。

### 修复方案

Assertion 不再接受公共 Candidate payload。Store transaction 内从 source candidate materialize：

```text
predicate
value_digest
scope
evidence
allowed_purposes
subject
expiry
impact
```

Promotion decision 只决定 lifecycle/status，不允许改变内容。DB trigger 对字段摘要做 exact equality。

---

## P0-12：Outcome-supported Human Model Promotion 没有绑定具体 Outcome

### 代码证据

- `RecordHumanModelPromotion` 的 OutcomeObservation proof 的 subject 是 promotion decision candidate 本身：`trust_chain.rs:437-448,530-542`。
- promotion payload validator 只检查 candidate digest、outcome string 和 evidence count：`1919-1942`。
- payload 没有 exact Outcome record ref/digest。
- `ProposeHumanModelCandidate` 只要求“存在一个 current Outcome”，Candidate 内容不必由该 Outcome 确定性派生。

### 可复现方式

1. Run 中存在任意合法 Outcome；
2. 提交与该 Outcome 无关的 Human Model candidate；
3. 提交 `promote_outcome_supported` decision；
4. OutcomeObservation issuer 对 promotion decision candidate 签名，而不是对 Outcome→Candidate derivation 签名。

系统无法证明该 Human Model claim 是由那个 Outcome 支持的。

### 修复方案

- `HumanModelCandidate` 必须保存 exact `outcome_ref/outcome_digest/observation_proof_ref`；
- Promotion proof subject 应是 `(candidate_ref, outcome_ref, derivation_digest)`；
- Promotion decision 内持久化这些 exact refs；
- assertion materialization再次复核。

---

## P0-13：Command ID 冲突会在任何验证前返回旧 Receipt

### 代码证据

- `postgres.rs:503-519` 先按 `command_id` 查询 receipt 并直接返回；
- 该分支发生在 DB time、run/tenant、Trust Root、proof、command digest 验证之前；
- `idr_command_receipts` 只保存 command ID、run ID、版本、事件序号和 receipt，不保存 canonical command digest/tenant/actor：`0001...sql:346-353`。
- `IdrCommandEnvelopeV1` 可 Deserialize，因此 command ID 可由 wire 重放。

### 可复现方式

1. 获取同一 tenant 中已成功 command 的 UUID；
2. 构造完全不同的 run/command/actor，但复用 command ID；
3. 发送给 Store。

Store 返回原 receipt，并标记 `idempotent_replay=true`，不会发现 command 内容冲突。调用者可能把其他操作的成功结果当作本次操作成功。

### 修复方案

- receipt 表保存 tenant、run、actor/caller、canonical command digest；
- idempotent replay 仅当新命令 canonical bytes 完全相同；
- 相同 command ID 不同 digest 返回 `IdempotencyConflict`；
- 查询分支也必须先校验 tenant/run binding。

---

## P0-14：读模型、inspect 和 outbox 控制 API 缺少认证与租户约束

### 代码证据

- `trust_chain.rs:777-782`：`inspect(run_id)` 不接受 tenant、principal 或 read proof。
- `postgres.rs:800-815`：按 run_id 直接加载完整 projection。
- Projection 暴露 permit ID、dispatch nonce、provider/owner、Human Model promotion 等。
- `postgres.rs:291-365`：Human Model query 的 tenant/subject/scope/purpose 全由调用者传入，没有认证上下文。
- `postgres.rs:367-421`：outbox claim 跨全部 tenant 选择事件，没有 tenant filter/worker credential。
- `postgres.rs:423-447`：任何调用者可用自选 worker_ref ack 自己抢到的事件。

### 可复现方式

持有 store/orchestrator 对象的普通业务模块可以：

- 枚举其他用户的有效 Human Model；
- inspect 其他 run 的 Permit/nonce；
- claim 所有租户 outbox；
- ack 后使合法消费者永远看不到这些事件。

### 修复方案

- 所有 read/control API 接受 `VerifiedReadContextV1`；
- tenant/subject 从认证证明中派生，不允许调用者直接选择；
- outbox worker 使用专用 service identity、tenant partition 和 DB role；
- projection read model去除 secret/permit fields；
- 产品 Human Model query 只能经 Runtime Query API。

---

# 6. P1 问题

## P1-01：没有生产级 WORM/KMS anchor 和 checkpoint signer

包内只有 `InMemoryAuditCheckpointAnchorV1`，明确 `is_production_durable=false`。checkpoint 的 `signed_checkpoint` 实际存储未签名结构。

## P1-02：Trust Root 是可注入、可由持有者直接 rotate 的进程内对象

`postgres.rs:31-69,130-156` 允许调用者注入任意 provider，并保留 `RotatingTrustRootProviderV1` clone 后随时 rotate。缺少受保护配置、KMS/签名配置发布和受审计的 root lifecycle。

## P1-03：External anchor 在 DB commit 之后发布，存在未锚定 tail

`postgres.rs:779-797` 先完成 `transact_inner()` commit，再 publish anchor。发布失败时数据库已经提交。`verify_external_anchor()` 允许 DB sequence 高于 anchor，因此 unanchored tail 在下一次成功 publish 前不受外部 rollback 保护。

## P1-04：Execution Permit 尚不是 provider-facing signed capability

当前 Permit 只存在 projection/DB 中，没有受 IDR protected key 签名的 provider envelope，也没有 provider adapter 的唯一消费 API。

## P1-05：Response 只有 send admission consumption，没有 delivery/fan-out Receipt

无法证明消息是否真正送达、送达哪些 recipient、重试/fan-out 如何结算。

## P1-06：Capability/Authority/Policy registry 和 issuer separation 不完整

expected issuer 是一张可注入 map；没有完整版本化 registry、policy hierarchy、delegation/revocation distribution，也没有防止同一 key 同时承担所有 proof kinds 的强制独立性。

## P1-07：Outcome conflict、归因和 Observation aggregation 仍过于简化

Outcome 只检查 receipt digest、observation text 和 observed digest，没有多 observation 冲突、来源新鲜度、attribution review 和 objective/success criteria 评估。

## P1-08：Human Model retention/sensitive inference/decay 仍未完成

没有完整敏感属性分类、目的限制变化、自动 decay/expiry worker、用户导出/删除证明、retention policy enforcement。

## P1-09：Audit replay 与 migration compatibility 未达到发布门槛

当前只有一套 migration；没有版本跨级升级/降级拒绝矩阵、长时间 fuzz、恢复期间 schema compatibility 和完整 event→projection replay verifier。

## P1-10：Aegis production adapter、生产 feature 和连续两轮审计仍关闭

这是正确的 fail-closed 状态，但表示 Round 6 仍不能成为 release candidate。

---

# 7. P2 问题

## P2-01：Round 5 Closure Matrix 结论过度

`ROUND5-CLOSURE-MATRIX.md:5,15,17` 将公共权威颁发、Human Model bypass、Orchestrator 控制面标为 Closed；P0-01、P0-11、P0-12 已证明这些尚未关闭。

## P2-02：API Visibility 文档与代码冲突

`API-VISIBILITY.md:13` 声称 PostgreSQL mutation 无公共 semantic write method，但 `PostgresIdrStoreV1::pool()` 公开完整 DML 能力。

## P2-03：Test Matrix 没有覆盖本轮关键攻击路径

缺失至少：

- malicious repository authority laundering；
- raw pool mutation；
- candidate cross-subject/cross-turn；
- Decision/Action selected option mismatch；
- successor without recursive invalidation；
- Cancelled run reserve/dispatch；
- same real operation/idempotency across Action revisions；
- Receipt request digest mismatch；
- HM assertion content substitution；
- conflicting command ID；
- cross-tenant inspect/HM/outbox。

## P2-04：TypeScript 仍保留旧式 Authorization Submission

`packages/interaction-client/src/index.ts:101-113,310-333` 仍生成无签名的 `{action_ref, parameter_digest, decision}` DTO。它可以作为 UI candidate，但命名容易让集成方误以为这是生产 Authorization。应明确改名为 `UserAuthorizationCandidateV1`，并由受认证 Gateway 转成 signed proof。

## P2-05：生产 Step 状态实现与文档不一致

独立的 orchestration step model 位于 legacy-shadow feature；PostgreSQL 生产路径把每个 step 都写成 completed。文档不应把它描述为完整 Orchestration Runtime。

## P2-06：交付可追溯性仍不理想

IDR 主目录没有 Git history；审计包 SHA 能固定快照，但无法证明 phase commits、review history 和变更责任链。Aegis 工作区还有大量既有 warnings，虽然与 IDR strict Clippy 不同，但应在正式集成前清理或建立 warning baseline。

---

# 8. 推荐修复顺序

## Phase 1：封死 Rust/Store 权威旁路

1. seal Repository trait；
2. transition function crate-private；
3. 删除 generic public Orchestrator constructor；
4. 删除 `pool()`；
5. authenticated inspect/HM/outbox APIs；
6. production DB 最小权限。

验收：downstream compile-fail 必须覆盖恶意 Repository、raw pool、direct transition。

## Phase 2：建立真正的 Aggregate/Lineage 门禁

1. centralized RunState/StepState matrix；
2. subject/turn/tenant/policy lineage；
3. terminal state全局拒绝；
4. successor 自动递归 invalidation；
5. projection replay/digest 校验。

## Phase 3：闭合 Decision → Action → Execution

1. ActionDerivationRecord；
2. selected option/operation/parameters exact mapping；
3. real operation idempotency key；
4. owner/provider lifecycle proof；
5. provider-facing signed Permit；
6. Receipt request digest exact binding。

## Phase 4：闭合 Outcome → Human Model

1. Outcome exact Action/Receipt/Observation relation；
2. Candidate exact Outcome derivation；
3. Promotion exact candidate+outcome binding；
4. Assertion 由 Store 内部 materialize；
5. authenticated effective query；
6. retention/sensitive/decay/delete。

## Phase 5：审计和部署基础设施

1. full-envelope audit hash；
2. projection root；
3. transactional anchor outbox；
4. WORM/KMS checkpoint signer；
5. protected Trust Root/registry distribution；
6. migration compatibility/replay/fuzz。

## Phase 6：重新验收

发布前至少新增以下动态攻击测试：

```text
malicious_repository_cannot_receive_authority
raw_pool_is_not_public
projection_tamper_detected
candidate_cannot_change_subject_or_turn
action_must_match_decision_selection
successor_atomically_invalidates_dependents
cancelled_run_cannot_reserve_or_dispatch
real_operation_idempotency_is_global_per_tenant
receipt_request_must_equal_action_request
human_model_assertion_cannot_change_candidate_content
outcome_supported_promotion_requires_exact_outcome
conflicting_command_id_is_rejected
cross_tenant_reads_and_outbox_claims_are_rejected
```

然后执行连续两轮真正独立的复审。只有两轮均没有新增 trust-boundary P0，才能考虑开启 production latch。

---

# 9. 最终门禁

```text
ROUND6_LOCAL_VALIDATION = PASS
ROUND6_INDEPENDENT_STATIC_REAUDIT = FAIL
ROUND6_FIRST_CLEAN_REVIEW = NO

IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

## 最终判断

Round 6 已经从“协议基线”进化到“有真实 PostgreSQL 垂直链路的安全原型”，这是重要进展。但它仍不是 Master Spec 所定义的生产信任根。

下一轮不需要再增加更多 Contract 类型。最高优先级是：

> **封死公共 Authority/Store 旁路，建立全局 Run/Lineage/Invalidation 门禁，再完成 Decision→Action 和 Outcome→Human Model 的精确派生。**
