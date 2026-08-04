# The Human-Centered Intent & Decision Runtime V1.3
# Trust-Chain Closure Master Specification

状态：**Normative / Implementation-Authoritative**  
适用基线：`IDR-V1.3-reaudit-round5-2026-07-29`  
目标：结束逐漏洞修补，将 IDR V1.3 重构为一条唯一、可证明、可持久、可恢复、不可绕过的生产信任链。  
正式名称：**The Human-Centered Intent & Decision Runtime（IDR）**  

---

## 0. 文档权威性

本文件是 IDR V1.3 Trust-Chain Closure 阶段的规范性实现合同。

优先级如下：

1. 本文件；
2. `source-design/IDR-V1.3-original-design.txt` 中不与本文件冲突的原始产品设计；
3. Round 5 现有代码和测试；
4. Round 5 之前的整改说明、实现说明和审计报告。

发生冲突时，必须以本文件为准。现有代码不能因为“已经实现”而反向修改本文件定义的信任边界。

本次不是继续修复 Round 5 的若干 P0，而是重构产生这些问题的共同根因：

- Rust 内部权威边界未封闭；
- Proof 没有形成统一信任模型；
- Orchestrator 不是唯一控制平面；
- Execution 不是一个完整聚合状态机；
- Store 不是生产级事务和审计基座；
- Outcome 与 Human Model 缺少真实证据链。

---

## 1. 系统目标与非目标

### 1.1 系统目标

IDR 将不可信的多源输入，转化为可审计的：

```text
Canonical Input
→ Intent
→ Decision
→ Turn Coordination
→ Response / Action
→ Authorization
→ Admission
→ Execution
→ Receipt
→ Outcome
→ Human Model Update
```

每次推进都必须满足：

- 输入来源可认证；
- 事实有来源；
- 决策有版本；
- 授权精确绑定；
- 执行资格唯一；
- 外部副作用可恢复或可对账；
- 结果与执行分离；
- Human Model 可纠正、可过期、可删除；
- 所有关键状态可重放和审计。

### 1.2 V1.3 生产范围

V1.3 必须实现：

- 单个逻辑租户内由 PostgreSQL 事务保证的强一致权威状态；
- 多进程实例共享同一数据库时的并发正确性；
- 一个租户同一聚合的单逻辑写入顺序；
- Gateway、Policy、Authority、Provider 等外部证明的密码学校验；
- 完整 Orchestration Runtime；
- 最小但闭合的生产垂直链路；
- Aegis Life 作为 reference/shadow host，并具备受约束的生产适配接口。

### 1.3 明确非目标

以下不属于 V1.3 首次 Trust-Chain Closure 的强制范围：

- 多区域 active-active；
- 在不支持幂等的任意外部系统上承诺绝对 exactly-once；
- 完整 Human Model 管理 UI；
- 任意 Cognitive Provider 的通用市场；
- 对所有业务领域完成 Capability 定义；
- 用 TypeScript 或 Python 承担任何生产裁决。

对于外部执行，IDR 只承诺：

- 每个幂等作用域最多一个活动 attempt；
- 不会自动并发 dispatch 两个 attempt；
- Provider 支持幂等时可安全重试；
- Provider 不支持幂等且结果不确定时，进入人工或自动 reconciliation，绝不盲目重试。

---

## 2. 语言职责边界

### 2.1 Rust：唯一生产信任根

Rust 负责：

- 线协议的权威解码与校验；
- Proof 验证；
- Trust Root 与撤销检查；
- Context Snapshot；
- Intent Fast Path；
- Decision Necessity；
- Response Admission；
- Action Admission；
- Human Model Update Gate；
- Contract 权威签发；
- Orchestration；
- Store 事务；
- Execution Reservation、Permit 和 Receipt；
- Outcome 与 Human Model 晋级；
- Audit Ledger 和 Outbox。

Rust 不等于天然可信。必须进一步区分：

```text
不可信 Rust 调用者 / Host / Adapter / Cognitive Service
                         ≠
IDR Orchestrator + Authoritative Issuer
```

任何普通 Rust crate 都不得直接构造、反序列化或提交权威 Contract。

### 2.2 TypeScript：产品与接入层

TypeScript 只允许：

- 构造并提交 Candidate/Input DTO；
- 展示 Runtime Read Model；
- 提交用户交互；
- 提交精确 Action Authorization 请求；
- 使用 Response Send Permit 渲染或发送；
- 使用 Execution Permit 调用受支持的产品侧执行适配器。

TypeScript 不得：

- 复制 Guard 规则；
- 自行判断 Authority/Policy；
- 签发权威 Contract；
- 自行声明 Action 已执行；
- 制造 Receipt、Outcome 或 Human Model Assertion。

### 2.3 Python：离线研究与评估

Python 只允许：

- fixture 回放；
- 模型、Prompt、规则候选评估；
- 发布前安全回归；
- 离线指标和研究。

Python 不得存在生产写入口、生产密钥、Store 写权限或 Authority 能力。

### 2.4 Aegis Life

Aegis Life 是 reference host，不是 IDR 的拥有者。

依赖方向必须永久保持：

```text
Aegis Life → IDR Adapter API → IDR Runtime
```

禁止：

```text
IDR Core → Aegis domain types
Aegis → direct authoritative Contract creation
Aegis → direct Store mutation
Aegis → direct Receipt / Outcome / Human Model write
```

---

## 3. 威胁模型与信任边界

以下全部默认不可信：

- 用户客户端；
- TypeScript SDK；
- Host；
- Agent；
- LLM / Cognitive Service；
- 外部工具；
- Provider 返回值；
- 传入的时间；
- 传入的布尔 facts；
- 反序列化的 JSON；
- 已在内存中验证但尚未消费的 Proof；
- 文件系统旧快照；
- 普通 Rust library caller；
- Aegis Life adapter。

以下是生产信任基座：

- IDR Orchestrator；
- Authoritative Issuer；
- 当前 Trust Root Snapshot；
- PostgreSQL 事务与约束；
- Trusted Clock；
- 已配置的外部 Audit Checkpoint Anchor；
- 经过当前 Trust Root 在线复核的 Proof；
- IDR Store 在同一事务中生成的 Admission/Permit/State Event。

---

## 4. 不可违反的全局不变量

以下不变量必须同时由类型、事务、数据库约束和攻击测试保证。

### 4.1 权威对象

1. 外部只能提交 Candidate、Request 或 Proof Envelope。
2. 权威 Contract 只能由 Orchestrator 内部的 Authoritative Issuer 创建。
3. 权威 Contract 不得暴露公共 `issue/new` 构造器。
4. 权威 Contract 不得从公共 wire JSON 直接 `Deserialize`。
5. Store 不接受调用者提供的“已权威”对象，只接受 Orchestrator Command。
6. 所有权威对象必须绑定 tenant、subject、run、turn、kind、ID、revision、digest、policy revision 和有效期。
7. successor 必须与 predecessor 的 kind、tenant、subject 和 lineage 规则一致。

### 4.2 Proof

8. 生产 Admission 不得消费调用者提供的裸布尔 facts。
9. 每个关键事实必须来自权威 Store、Context Snapshot 或 Verified Proof。
10. Proof 必须绑定 issuer、key、proof type、audience、tenant、subject、目标对象摘要、nonce、时间窗口和 policy revision。
11. Proof 必须在**消费事务内部**使用当前 Trust Root 和 Trusted Clock 重新验证。
12. 已撤销、过期、audience 不匹配或 nonce 已消费的 Proof 必须 fail closed。
13. `VerifiedProof<T>` 是进程内临时能力，不可序列化、不可由调用者构造。
14. 持久化的是原始 Proof Envelope 和 Verification/Consumption Record，不是可复用的 `VerifiedProof<T>`。

### 4.3 Intent 与 Decision

15. Base Intent Resolution 不得读取长期 Human Model。
16. Human Model 只能重排、追加或暴露冲突，不得删除 Base Hypotheses。
17. Fast Path 只能由 `IntentFastPathAdmission=ALLOW` 启用。
18. `UNKNOWN` 必须进入 Full Path；`BLOCK` 必须终止当前意图推进。
19. Decision Necessity 的 `UNKNOWN` 必须进入 Decision Runtime。
20. 不进入 Decision Runtime 时必须存在精确 `NoDecisionRequiredAdmission`。
21. Action 若来自 Decision，必须存在 Action Derivation Record，证明 selected option 与 operation/parameters 的确定性映射。

### 4.4 Response

22. Response Contract 与实际 rendered bytes 必须精确绑定。
23. Response Policy Proof 必须绑定 response ref、content digest、audience、channel、authority scope 和 policy revision。
24. Host 只能持有一次性或显式 fan-out 的 Response Send Permit，不能直接发送未准入内容。
25. Send Permit 过期、失效或已消费时不得重放。

### 4.5 Action、Authorization 与 Execution

26. Action Contract 不代表执行资格。
27. User Authorization 必须精确绑定 Action ID、kind、revision、record digest、parameter digest、tenant、subject、actor、operation、scope、purpose和有效期。
28. User Authorization 必须来自认证 Gateway/Auth issuer 的 Proof，不能由普通 Rust 调用者自签发。
29. Action Admission 必须成为建立 Execution Reservation 的唯一前置证明。
30. 不存在 legacy claim、旁路 Permit 或直接 Executor 调用路径。
31. 同一 `(tenant, operation, idempotency_key)` 最多存在一个活动 attempt。
32. Permit 必须绑定 Action Admission、Authorization（若需要）、Capability、Policy、provider、owner、attempt、nonce、lease 和 Trust Root version。
33. Action 在 dispatch 前失效时，未 dispatch 的 Reservation 必须在同一事务中取消并撤销 Permit。
34. Action 在 dispatch 后失效时，不得抹去执行事实；必须进入 stale/unknown reconciliation，禁止自动重复执行。
35. lease 在 dispatch 前过期时，状态必须事务性持久化为 Expired，允许合法下一 attempt。
36. lease 在 dispatch 后过期且无 Receipt 时，必须进入 UnknownNeedsReconciliation，不得自动重试。
37. Receipt 必须绑定 Permit、dispatch、provider、attempt、request digest、result digest 和 Provider Proof。
38. Receipt 即使晚到，也必须按真实 dispatch 接收和审计；不能因为 Action 后续失效而丢弃真实执行事实。
39. 不支持 Provider 幂等且执行结果未知时，禁止自动再次 dispatch。

### 4.6 Outcome 与 Human Model

40. Execution Receipt 与 Business Outcome 永久分离。
41. Outcome 必须来自 Observation Proof，不能由调用者填写字符串后直接晋级。
42. Outcome 必须验证其 Receipt、Action、Decision、tenant、subject 和时间关系。
43. Human Model Candidate、Promotion Decision 和 Assertion 必须是三个不同对象。
44. Promotion Decision 必须绑定 exact candidate digest、subject、predicate、value digest、scope 和证据。
45. `USER_CONFIRMED` 必须绑定用户来源的 Confirmation Proof。
46. `OUTCOME_SUPPORTED` 必须绑定 exact Outcome Record 和 Observation Proof。
47. Cognitive Service 最多产生 Candidate，不能晋级。
48. 敏感属性推断默认拒绝。
49. 读取 Human Model 必须经过 Runtime Query API，强制执行 lifecycle、expiry、usage policy 和 impact limit；业务代码不得绕过读取 raw assertion。
50. 用户纠正、拒绝和删除必须产生不可变事件并改变 effective view。

### 4.7 Store 与审计

51. 生产权威 Store 使用 PostgreSQL；现有 whole-file Store 仅允许 dev/test/shadow。
52. 生产事务必须原子写入 aggregate event、snapshot/current pointer、依赖索引、proof consumption、outbox 和 audit event。
53. Contract 唯一键至少为 `(tenant, kind, contract_id, revision)`；同 revision 不允许不同 digest。
54. 依赖图必须无环，递归失效闭包必须可由历史记录重算并验证。
55. 所有命令必须携带 command ID 和 expected aggregate version，实现幂等与 CAS。
56. Audit Ledger 必须 append-only、哈希链、可重放。
57. 定期 Audit Checkpoint 必须由 IDR key 签名并发送到数据库之外的 Anchor Backend。
58. production 启动时若没有 Trust Root、Trusted Clock、Postgres Store、Checkpoint Anchor 或 Crypto Proof，必须拒绝启动。

---

## 5. 完整组件架构

```text
External Sources
  ↓
Authenticated Protocol Gateway
  ↓ Canonical Input Candidate + Input Proof
IDR Orchestrator
  ├─ Context Fabric
  ├─ Human Model Query Runtime
  ├─ Intent Fast Path Admission
  ├─ Base Intent / Personalization
  ├─ Decision Necessity Admission
  ├─ Decision Collaboration Runtime
  ├─ Response & Action Planner
  ├─ Turn Coordination Admission
  ├─ Response Admission / Send Permit
  ├─ Action Admission
  ├─ Execution Reservation / Permit Runtime
  ├─ Receipt Commit / Reconciliation
  ├─ Outcome Runtime
  └─ Human Model Update Gate
        ↓
PostgreSQL Authoritative Store
  ├─ Event Store
  ├─ Contract Store
  ├─ Dependency / Invalidation Index
  ├─ Proof Registry / Consumption
  ├─ Execution Aggregates
  ├─ Human Model Versions
  ├─ Audit Hash Chain
  └─ Transactional Outbox
        ↓
Outbox Workers / Host Adapters / Providers
        ↓
Signed Receipts / Observations
```

Cognitive Services 只能通过 ports 返回候选：

```text
IntentCandidate
DecisionCandidate
ResponseCandidate
ActionCandidate
AttributionCandidate
HumanModelUpdateCandidate
```

候选必须由 Rust 校验、裁决和签发后才能进入权威状态。

---

## 6. 统一 Proof Framework

### 6.1 Proof Envelope

所有生产 Proof 使用同一个 envelope：

```rust
ProofEnvelopeV1 {
    proof_id,
    proof_type,
    issuer_ref,
    key_id,
    algorithm,
    audience_ref,
    tenant_ref,
    subject_ref,
    object_ref,
    object_digest,
    policy_revision_ref,
    session_ref: Option,
    nonce,
    issued_at,
    not_before,
    expires_at,
    payload_digest,
    signature,
}
```

签名输入必须使用明确 domain separation：

```text
IDR-PROOF-V1 || proof_type || canonical_payload_bytes
```

### 6.2 Canonical Encoding

所有 digest 和签名统一使用 **RFC 8785 JSON Canonicalization Scheme（JCS）** 或仓库内等价、经过固定向量验证的规范编码。

约束：

- 禁止浮点数；置信度使用 basis points；
- V1.3 wire 时间统一使用整数 Unix seconds；任何更高精度的数据库时间只可用于
  内部排序，不得直接进入跨语言签名对象；
- map key 使用 canonical order；
- UTF-8 与 Unicode 处理必须有固定测试；
- Rust、TypeScript、Python 对同一 fixture 必须产生相同字节和 digest；
- 所有摘要必须有 domain tag。

### 6.3 Trust Root

`TrustRootSnapshotV1` 至少包含：

- version；
- accepted issuers；
- key material / KMS references；
- key validity windows；
- revocation records；
- proof type permissions；
- audience restrictions；
- loaded_at；
- snapshot signature/digest。

每次 Proof 消费都必须在 Store 事务中重新加载或锁定当前 Trust Root version。

### 6.4 Trusted Clock

生产时间不得由 API 调用者提供。

PostgreSQL 事务使用数据库可信时间作为消费时刻；应用层 `TrustedClock` 只可用于测试和非权威预检查。所有最终过期、lease、revocation 和 admission 判定必须使用事务内时间。

### 6.5 必需 Proof 类型

- `InputSourceProofV1`
- `AuthorityProofV1`
- `PolicyDecisionProofV1`
- `UserAuthorizationProofV1`
- `CapabilityProofV1`
- `ResponsePolicyProofV1`
- `ProviderReceiptProofV1`
- `OutcomeObservationProofV1`
- `HumanModelUserConfirmationProofV1`
- `AuditCheckpointProofV1`

Proof type 必须限制哪些 issuer/key 可以签发。

---

## 7. 权威对象与公共 DTO 分离

每个领域对象分为三层：

```text
Public Candidate DTO
→ Validated Candidate
→ Authoritative Record
```

### 7.1 Public Candidate DTO

允许 wire `Deserialize`，只表达外部候选，不具备权限。

### 7.2 Validated Candidate

由 Rust validator 生成，证明 shape、schema、资源预算和基础语义合法，但仍不是权威 Contract。

### 7.3 Authoritative Record

必须：

- 位于内部模块或 crate-private；
- 不对下游 crate 暴露公共构造器；
- 不允许公共 wire `Deserialize`；
- 只能由 Authoritative Issuer 在 Orchestrator transaction context 中创建；
- 对外只暴露 immutable read model 和 exact reference。

Store replay 使用内部 `StoredEnvelopeV1`，不得复用公共 DTO 解码路径。

必须添加 `trybuild` compile-fail tests，证明外部 crate 无法：

- 调用 authoritative `issue/new`；
- 从 JSON 反序列化 authoritative record；
- 直接提交 Store mutation；
- 构造 `VerifiedProof<T>`；
- 直接生成 Permit、Receipt、Outcome 或 Assertion。

---

## 8. Orchestration Runtime

### 8.1 唯一写入口

生产代码只能通过：

```rust
IdrOrchestrator::handle(command, auth_context)
```

修改权威状态。

模块不得直接写 Store。模块返回：

```text
Candidate
Proposed Event
Required Input
Failure
```

Orchestrator 决定是否签发、等待、失败、重试或失效。

### 8.2 Command Envelope

```rust
OrchestratorCommandEnvelopeV1 {
    command_id,
    run_id,
    expected_run_version,
    tenant_ref,
    caller_proof,
    command,
}
```

同一 `command_id` 必须幂等返回相同结果。

### 8.3 Run 状态

```text
Pending
Running
WaitingInput
WaitingAuthorization
WaitingDependency
WaitingExecution
WaitingObservation
Succeeded
Failed
Cancelled
TimedOut
Invalidated
```

### 8.4 Step 状态

```text
Pending
Ready
Running
Waiting
Completed
Failed
Cancelled
Invalidated
```

### 8.5 核心命令

- `IngestInput`
- `BuildContextSnapshot`
- `ResolveIntent`
- `EvaluateDecisionNecessity`
- `BuildDecision`
- `PlanTurn`
- `RenderAndAdmitResponse`
- `SubmitUserAuthorization`
- `AdmitAction`
- `ReserveExecution`
- `RecoverPermit`
- `MarkPermitDelivered`
- `MarkDispatchStarted`
- `CommitExecutionReceipt`
- `ReconcileUnknownExecution`
- `RecordOutcomeObservation`
- `EvaluateHumanModelCandidate`
- `ApplyUserCorrection`
- `CancelRun`
- `HandleTimeout`
- `RecomputeFromInvalidation`

### 8.6 失效传播

新事实或上游修订出现时：

1. 在同一事务内写入新 revision；
2. 计算并写入依赖旧 exact ref 的递归失效闭包；
3. 标记需要局部重算的 step；
4. 对未 dispatch Reservation 执行取消；
5. 对已 dispatch 执行标记 stale/needs reconciliation；
6. 通过 Outbox 安排重算、取消或对账。

不得只更新 Contract current pointer 而不处理活动执行状态。

---

## 9. Intent、Decision 与 Turn 的强制 Admission

### 9.1 Context Snapshot

`ContextSnapshotV1` 必须绑定：

- exact input refs；
- Authority Proof；
- Domain Facts refs 与 freshness；
- Session state；
- Human Model effective assertion refs；
- policy revision；
- snapshot time；
- snapshot digest。

### 9.2 Intent Fast Path Admission

不再公开接受 `IntentFastPathFactsV1`。

生产 API 只接受 `ContextSnapshotV1` 和已注册 Command Schema，由内部 Fact Builder 形成 facts。

Admission Record 绑定：

- input refs；
- context snapshot ref；
- command schema revision；
- facts digest；
- outcome；
- reason codes；
- rule version；
- policy revision；
- issued_at / valid_until。

`IntentContract.resolution_method=DeterministicFastPath` 时必须依赖精确 `ALLOW` Admission。

### 9.3 Decision Necessity Admission

同样由内部 Fact Builder 从 Intent、Context、Impact 和 Policy 生成。

输出：

- `DecisionRequiredAdmissionV1`；或
- `NoDecisionRequiredAdmissionV1`。

Action 必须依赖其中一个，不能没有 Decision 证明。

### 9.4 Action Derivation

若 Decision 存在，Action 必须有：

```text
ActionDerivationRecordV1
```

它绑定：

- decision ref；
- selected option ref；
- compiler/rule revision；
- operation ref；
- parameter digest；
- satisfied constraints；
- unresolved constraints；
- evidence refs。

若不需要 Decision，则 Action 依赖 `NoDecisionRequiredAdmissionV1` 和 deterministic command mapping。

### 9.5 Turn Coordination Admission

Turn mode 不允许由调用者直接选择。由内部 selector 根据 Response/Action、authorization、long-running 和 parallel safety 事实生成 Admission，再签发 Turn Coordination Plan。

Plan 必须是 DAG；每个 Gate 绑定 exact Action；模式与依赖边必须一致。

---

## 10. Response Admission 与发送

### 10.1 流程

```text
Response Candidate
→ Response Contract
→ Host Renderer returns rendered bytes
→ Rust recomputes content digest
→ Response Policy Service returns signed proof
→ Rust Response Admission
→ Response Send Reservation / Permit
→ Adapter sends
→ Delivery Receipt
```

### 10.2 Response Send Permit

必须绑定：

- response ref；
- content digest；
- audience；
- channel；
- tenant/subject；
- authority scope；
- policy proof ref；
- renderer ref；
- issued_at / expires_at；
- send nonce；
- allowed delivery count。

默认只能消费一次。多 channel fan-out 必须在 Permit 中明确列出每个 channel 和次数。

### 10.3 禁止路径

- Adapter 直接发送只有 Response Contract、没有 Send Permit 的内容；
- 客户端自报 policy facts；
- 调用者提供最终发送时间；
- 修改 rendered bytes 后复用旧 Permit。

---

## 11. Action Admission

Action Admission 是 Execution Reservation 的唯一入口。

必须在同一事务中验证：

- Action current、未失效、未过期；
- dependencies current；
- Decision/NoDecision Admission；
- Action Derivation；
- Capability Proof；
- actor 与 agent Authority Proof；
- parameter schema；
- preconditions；
- current Policy Proof；
- exact User Authorization（若要求）；
- verification 能力；
- compensation 能力；
- idempotency scope；
- 当前没有冲突活动 Reservation。

输出 `ActionAdmissionDecisionV1`，至少包含：

- exact action ref；
- authorization ref；
- derivation ref；
- capability proof ref；
- authority proof refs；
- policy proof ref；
- precondition snapshot ref；
- idempotency scope；
- outcome；
- reason codes；
- trust root version；
- issued_at / valid_until；
- admission digest。

只有 `Admit` 可原子创建 Reservation。

---

## 12. Execution Aggregate 状态机

### 12.1 Aggregate Key

```text
(tenant_ref, operation_ref, idempotency_key)
```

同时索引 exact Action ref。

### 12.2 Attempt 状态

```text
Reserved
PermitIssued
Delivered
DispatchStarted
AwaitingReceipt
Succeeded
FailedRetryable
FailedTerminal
Rejected
Cancelled
ExpiredBeforeDispatch
InvalidatedBeforeDispatch
UnknownNeedsReconciliation
Compensating
Compensated
```

### 12.3 允许迁移

```text
Reserved → PermitIssued
PermitIssued → Delivered
Delivered → DispatchStarted
DispatchStarted → AwaitingReceipt
AwaitingReceipt → Succeeded | FailedRetryable | FailedTerminal | Rejected | UnknownNeedsReconciliation

Reserved | PermitIssued | Delivered
  → ExpiredBeforeDispatch | InvalidatedBeforeDispatch | Cancelled

DispatchStarted | AwaitingReceipt
  → UnknownNeedsReconciliation

FailedRetryable | Rejected | ExpiredBeforeDispatch | InvalidatedBeforeDispatch
  → next attempt: Reserved

FailedTerminal | Succeeded | Compensated
  → no normal retry

UnknownNeedsReconciliation
  → Succeeded | FailedRetryable | FailedTerminal | Compensating | human-reviewed terminal
```

### 12.4 Permit

Permit 必须由 Store transaction 生成，并绑定：

- permit ID；
- exact Action Admission；
- exact Action；
- exact Authorization（若有）；
- provider；
- owner；
- attempt；
- lease；
- dispatch nonce；
- idempotency key；
- request parameter digest；
- trust root version；
- issued event sequence；
- signature。

### 12.5 Action 失效处理

- `Reserved/PermitIssued/Delivered`：同一事务转 `InvalidatedBeforeDispatch`，Permit 不再可用，允许新 Action lineage 创建新的 reservation。
- `DispatchStarted/AwaitingReceipt`：不得取消事实；写 `ActionInvalidatedAfterDispatch`，转 `UnknownNeedsReconciliation` 或保留 awaiting receipt + stale flag。
- Receipt 晚到时仍接收并记录，但 Outcome 必须披露执行基于已失效 Action。

### 12.6 Lost Permit 恢复

- lease 未过期：返回同一 Permit，不生成新 Permit；
- lease 已过期且未 dispatch：在恢复事务中先持久化 `ExpiredBeforeDispatch`，再允许显式创建下一 attempt；
- 已 dispatch：不得按过期处理，进入 receipt/reconciliation 流程。

### 12.7 Crash 边界

必须通过事务顺序保证：

- Proof 消费与 Reservation 创建不可分离；
- Reservation 已提交但 Permit 响应丢失时可恢复相同 Permit；
- DispatchStarted 必须在调用 Provider 前持久化；
- Provider 已执行但 Receipt 丢失时进入 reconciliation，不盲目重试；
- Receipt commit 与 terminal transition、outbox、audit 同事务。

### 12.8 Provider

Provider dispatch 必须携带：

- Permit；
- idempotency key；
- request digest；
- dispatch nonce。

Provider Receipt Proof 必须绑定：

- permit ID；
- dispatch nonce；
- provider execution ID；
- request digest；
- result digest；
- attempt；
- timestamps；
- status；
- provider key。

---

## 13. Outcome Runtime

### 13.1 Execution Result 与 Outcome 分离

Receipt 证明外部调用发生了什么；Outcome 证明用户或业务目标后来发生了什么。

### 13.2 Observation Proof

Outcome 只能由以下可信观察来源推进：

- Provider；
- Domain Monitor；
- Human Approver/User；
- Authorized External System。

`OutcomeObservationProofV1` 必须绑定：

- subject；
- action/receipt/decision refs；
- observation type；
- observed value digest；
- evidence refs；
- observed_at；
- issuer；
- confidence；
- nonce；
- policy revision。

### 13.3 Attribution

Cognitive Service 可提出 Attribution Candidate，但最终 Outcome Record 必须由 Rust 校验：

- 当前是否有足够证据；
- 是否存在冲突观察；
- 是否允许归因；
- 是否需要人工复核。

没有足够证据时必须是 `Unknown/InsufficientEvidence`，不能强行归因。

---

## 14. Human Model Runtime

### 14.1 三对象分离

```text
HumanModelUpdateCandidateV1
→ HumanModelPromotionDecisionV1
→ HumanModelAssertionV1
```

### 14.2 Candidate

Candidate 可以来自 Cognitive Service，但必须绑定：

- subject；
- predicate；
- value digest；
- scope；
- source refs；
- evidence refs；
- proposed persistence；
- proposed usage；
- candidate digest。

### 14.3 Promotion Decision

必须精确绑定 Candidate，不能只保存一个通用 Gate outcome。

允许：

- `KeepSession`
- `StoreCandidate`
- `PromoteProvisional`
- `PromoteUserConfirmed`
- `PromoteOutcomeSupported`
- `Reject`

规则：

- UserConfirmed：需要 exact User Confirmation Proof；
- OutcomeSupported：需要 exact Outcome + Observation Proof；
- Provisional：需要规定数量的独立证据，且来源去重；
- contradiction 或敏感推断：Reject/Review；
- Candidate 不能直接长期生效。

### 14.4 Assertion

Assertion 由 Store transaction 内部签发并版本化。不得公共反序列化或直接提交。

### 14.5 读取

业务读取只能调用：

```rust
HumanModelRuntime::query_effective_assertions(query, trusted_context)
```

强制执行：

- subject/tenant；
- lifecycle；
- expiry/decay；
- correction；
- usage policy；
- maximum impact；
- domain/task scope；
- policy revision。

Raw assertion 只允许审计管理员通过独立权限读取。

---

## 15. PostgreSQL 权威 Store

### 15.1 生产与开发 Store

- `PostgresIdrStore`：唯一生产实现；
- `FileIdrStore`：仅 dev/test/shadow，编译时不得与 `production` feature 共存；
- production 启动时检测到 file store 必须失败。

### 15.2 最低表结构

- `idr_runs`
- `idr_run_events`
- `idr_step_snapshots`
- `idr_contract_records`
- `idr_contract_current`
- `idr_contract_dependencies`
- `idr_contract_invalidations`
- `idr_proof_envelopes`
- `idr_proof_consumptions`
- `idr_execution_reservations`
- `idr_execution_attempts`
- `idr_execution_receipts`
- `idr_outcome_records`
- `idr_human_model_candidates`
- `idr_human_model_assertions`
- `idr_audit_events`
- `idr_outbox_events`
- `idr_audit_checkpoints`

### 15.3 数据库约束

必须使用唯一约束和外键保证：

- Contract revision 不分叉；
- predecessor 连续；
- proof nonce/ID 不重复消费；
- 一个 idempotency scope 最多一个活动 attempt；
- attempt 连续递增；
- Receipt 只能对应已 DispatchStarted 的 attempt；
- Human Model Assertion 必须对应 Promotion Decision；
- command ID 幂等；
- aggregate version CAS。

### 15.4 事务与锁

关键写入使用 `SERIALIZABLE` 或等价 row/advisory locking。

每个命令在一个数据库事务中完成：

```text
load aggregate + current trust root + DB time
→ verify command/proofs/currentness
→ append events
→ update read snapshots/indexes
→ write audit event
→ write outbox
→ commit
```

### 15.5 Transactional Outbox

所有外部发送、Provider dispatch、取消、重算和 checkpoint publish 均由 outbox 驱动。

禁止数据库状态已提交但外部消息没有可靠记录。

### 15.6 Audit Ledger

Audit Event 至少包含：

- sequence；
- event ID/type；
- tenant/aggregate/version；
- actor/caller；
- correlation/causation；
- contract/proof refs；
- policy/model/prompt/rule versions；
- previous hash；
- event hash；
- payload ref；
- DB timestamp。

### 15.7 Anti-Rollback

数据库内部 hash chain 不能单独防止整库回滚。

必须实现 `AuditCheckpointAnchor`：

- 定期生成包含 tenant/global latest sequence 与 chain root 的签名 checkpoint；
- 发布到数据库之外的 append-only/WORM/KMS-backed backend；
- 启动和周期检查时比较外部 anchor；
- 发现 DB sequence/root 落后于 anchor 时拒绝生产写入。

生产配置没有 Anchor Backend 时拒绝启动。

---

## 16. Capability Registry、Policy 与 Authority

### 16.1 Capability Registry

每项 capability 必须定义：

- operation ref/version；
- provider；
- parameter schema；
- result schema；
- impact；
- reversible；
- authority requirements；
- verification requirements；
- compensation；
- idempotency support；
- timeout/retry policy；
- allowed tenants；
- status/expiry。

### 16.2 Policy Hierarchy

```text
Immutable Safety Rules
> Platform Policy
> Tenant Policy
> User Preference
```

下层只能收紧上层，不能放宽。

### 16.3 Authority

Authority Proof 必须明确：

- subject；
- actor；
- caller；
- tenant；
- scope；
- purpose；
- operation；
- constraints；
- grant refs；
- issuer；
- validity。

Action Admission 执行结构化包含关系检查，禁止使用 `actor_authority_valid: bool` 等外部字段。

---

## 17. 跨语言协议

### 17.1 单一 Schema 源

Rust schema 是权威源，但必须自动生成：

- JSON Schema；
- TypeScript types + validators；
- Python models/evaluation DTO；
- canonical fixture；
- digest/signature golden vectors。

禁止长期维护三份手写枚举和字段集合。

### 17.2 兼容策略

每个 wire envelope 包含：

- schema version；
- minimum reader version；
- feature flags；
- unknown critical field policy。

破坏性变化必须新 major schema，不能静默复用 V1。

### 17.3 Conformance CI

CI 必须验证：

- 三语言解码结果一致；
- unknown fields 处理一致；
- integer/safe range 一致；
- JCS bytes 一致；
- digest 一致；
- signature verification 一致；
- fixture expected behavior 一致。

---

## 18. Feature Gates 与生产启动门禁

引入明确 features：

```text
production
postgres-store
crypto-proofs
trusted-clock
orchestrator
transactional-outbox
audit-anchor
human-model-write
aegis-production-adapter
dev-file-store
shadow-mode
```

规则：

- `production` 必须包含前七项；
- `production + dev-file-store` 编译失败；
- `human-model-write` 只有 Human Model 全部 P0 测试通过才可启用；
- `aegis-production-adapter` 只有完整端到端测试通过才可启用；
- 缺少生产 Trust Root/Anchor/DB migration 时启动失败；
- README 声明不能替代编译和运行门禁。

---

## 19. Round 5 审计问题的结构性关闭映射

以下问题必须通过本规范整体关闭，而不是局部添加判断：

1. 权威 Contract 颁发与持久化未封闭 → 第 7、8、15 节。
2. Guard facts 由调用者控制 → 第 6、9、11 节。
3. Canonical Input 无来源认证与防重放 → 第 6、9 节。
4. Authorization 不是可信用户证明 → 第 6、11、16 节。
5. Action Admission 不是 Permit 唯一前置 → 第 11、12 节。
6. Store 消费时不检查当前 Trust Root → 第 6、15 节。
7. Action 失效后旧 Permit 可 dispatch → 第 8、12 节。
8. Lost Permit 过期后永久锁死 → 第 12 节。
9. Receipt Proof context/时间复核不足 → 第 6、12 节。
10. Response Admission 可自报和重放 → 第 10 节。
11. Human Model Gate 可绕过/不绑定内容 → 第 14 节。
12. Outcome 无 Observation Attestation → 第 13 节。
13. Orchestrator 不是控制平面 → 第 8 节。
14. Store anti-rollback 仅本机 → 第 15 节。

---

## 20. 实施顺序

Codex 可以分阶段提交，但必须在一个连续任务中自动推进，不得每完成一个阶段就要求用户重新定义架构。

### Phase A：冻结与边界封闭

- 建立新分支；
- 保存 Round 5 baseline；
- 增加 production feature compile gates；
- 分离 Candidate DTO / Authoritative Record；
- 封闭构造器、Deserialize 和 Store mutation；
- 加 compile-fail tests。

### Phase B：Proof、Trust Root、Trusted Clock、Canonical Encoding

- 统一 Proof Envelope；
- 实现 JCS；
- 实现当前 Trust Root 在线验证和撤销；
- 实现 DB transaction time；
- 实现 Input/Authority/Policy/Authorization proof。

### Phase C：真正 Orchestrator 与强制 Admission

- 实现 Run/Step event-sourced aggregate；
- 实现 command ID 幂等、version CAS；
- 实现 Context Snapshot；
- 把 Intent/Decision/Turn/Response/Action Admission 串为唯一路径。

### Phase D：PostgreSQL Store、Audit、Outbox、Anchor

- SQL migrations；
- Postgres repository；
- constraints；
- transactional outbox；
- audit chain；
- external checkpoint anchor interface + production fail-closed implementation path；
- file store 降级为 dev-only。

### Phase E：Execution Aggregate 重构

- 按第 12 节实现完整状态机；
- 修复 invalidation；
- 修复 lost/expired permit；
- dispatch-before-side-effect；
- Provider Receipt Proof；
- reconciliation；
- compensation。

### Phase F：Response、Outcome、Human Model 闭环

- Response Send Permit；
- Observation Proof；
- Outcome；
- Candidate/Promotion/Assertion；
- effective query boundary；
- correction/deletion/retention。

### Phase G：跨语言生成与 Aegis

- schema generation；
- TS/Python conformance；
- Aegis adapter 只调用公共 Orchestrator/Gateway/Permit API；
- shadow 与 production feature 分离。

### Phase H：攻击、并发、崩溃和发布验收

- fuzz/property/trybuild；
- PostgreSQL concurrency；
- crash injection；
- outbox/recovery；
- key revocation；
- anti-rollback；
- full end-to-end vertical slice。

---

## 21. 强制测试矩阵

至少新增以下自动化测试：

### 21.1 API 与反序列化

- 外部 crate 无法构造 authoritative Contract；
- 无法 deserialize authoritative record；
- 无法构造 VerifiedProof、Admission、Permit、Receipt、Outcome、Assertion；
- malformed/newtype/unknown fields/fuzz 全部 fail closed。

### 21.2 Proof

- wrong issuer/key/audience/tenant/subject/object digest；
- expired/not-before/revoked key；
- nonce replay；
- proof 预验证后 key 撤销再消费；
- trust root version 更新；
- JCS cross-language vectors。

### 21.3 Intent/Decision/Turn

- Fast Path 无 Admission 不能签发 Intent；
- Unknown 强制 Full Path；
- Decision Required 不能绕过；
- Action 与 selected option 不匹配；
- Turn cycle、错误 Gate、错误 mode。

### 21.4 Response

- rendered bytes 修改；
- policy proof 与 digest 不匹配；
- Send Permit 重放；
- channel/audience 错误；
- 过期后发送；
- fan-out 超次数。

### 21.5 Execution

- 两个 Authorization 并发创建同 idempotency Reservation；
- Action 失效前后 dispatch；
- Permit 返回前崩溃；
- lost Permit 在 lease 前/后恢复；
- dispatch 后 lease 到期；
- Provider 成功但 Receipt 丢失；
- Receipt 晚到；
- Receipt wrong permit/provider/nonce/attempt；
- revoked provider key；
- unknown execution 不自动 retry；
- retry attempt 连续性；
- compensation；
- 多进程并发。

### 21.6 Outcome/Human Model

- Receipt 不能冒充 Outcome；
- Outcome/Receipt/Action mismatch；
- 无 Observation Proof；
- Human Model Gate 复用到不同 candidate；
- 伪造 UserConfirmed；
- unrelated OutcomeSupported；
- sensitive inference；
- expired/rejected/corrected assertion 读取；
- usage policy 和 impact limit。

### 21.7 Store/Audit

- transaction rollback；
- outbox crash recovery；
- duplicate command；
- revision fork；
- dependency cycle；
- invalidation replay；
- DB rollback behind external anchor；
- migration compatibility；
- restart deterministic replay。

---

## 22. 最小生产垂直链路

最终必须有一个不依赖 Aegis 业务细节的集成测试，完整执行：

```text
Authenticated User Input Proof
→ Canonical Input Event
→ Context Snapshot
→ Intent Fast Path UNKNOWN
→ Full Intent Contract
→ Decision Required
→ Decision Contract
→ Response Contract
→ Response Policy Proof
→ Response Send Permit consumed
→ Action Contract
→ Exact User Authorization Proof
→ Action Admission Admit
→ Execution Reservation
→ Permit Issued / Delivered / DispatchStarted
→ Signed Provider Receipt
→ Execution Receipt committed
→ Signed Outcome Observation
→ Outcome Record
→ Human Model Candidate
→ Promotion Decision
→ Effective Assertion query
```

测试必须同时覆盖：

- happy path；
- authorization deny；
- Action invalidation before dispatch；
- crash before Permit response；
- unknown after dispatch；
- revoked proof；
- response replay；
- Human Model promotion bypass。

---

## 23. 完成门槛

只有同时满足以下条件，才允许：

```text
PRODUCTION TRUST ROOT = PASS
```

门槛：

1. 本文全部 MUST/不得 条款已实现；
2. P0 = 0；
3. 安全、一致性和恢复类 P1 = 0；
4. production feature 不包含 file store 或 mock trust components；
5. 所有生产权威状态只能经 Orchestrator；
6. 不存在裸 facts Admission；
7. 不存在第二条执行资格通道；
8. Action invalidation、lease、crash、reconciliation 全部有动态测试；
9. Postgres concurrency/crash tests 通过；
10. Rust workspace tests、strict Clippy、rustfmt 通过；
11. TypeScript tests/typecheck 通过；
12. Python tests 通过；
13. cross-language canonical/digest/signature vectors 通过；
14. Aegis dependency and privilege monotonicity tests 通过；
15. Audit external anchor rollback test 通过；
16. 连续两轮独立复审没有发现新的信任边界级 P0。

Human Model 长期写入和 Aegis production adapter 有独立门禁；不能因为核心链路通过而自动开启。

---

## 24. Codex 最终交付物

Codex 最终必须提交：

- 完整代码；
- 数据库 migrations；
- 架构文档；
- 状态机文档；
- Proof binding 表；
- API visibility 表；
- threat model；
- test matrix；
- migration guide；
- Round 5 问题关闭矩阵；
- validation summary；
- 未完成项和剩余风险。

最终报告不得用“类型已经存在”“README 已声明”作为通过证据。每一项通过必须引用：

- 具体代码；
- 数据库约束；
- 自动化测试；
- 实际命令结果。
