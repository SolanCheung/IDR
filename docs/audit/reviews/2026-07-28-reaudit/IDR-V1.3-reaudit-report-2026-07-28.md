# The Human-Centered Intent & Decision Runtime V1.3 复审报告

审计对象：`IDR-V1.3-reaudit-2026-07-28.zip`  
审计日期：2026-07-28  
系统正式名称：**The Human-Centered Intent & Decision Runtime**  
简称：**IDR**  
审计性质：整改包差异复审、完整静态源码审计、可执行测试复核

---

# 一、总体结论

## 1. 最终判定

```text
PRODUCTION TRUST ROOT = BLOCKED
FOUNDATION / SHADOW MODE = ACCEPTABLE WITH LIMITATIONS
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
```

本轮整改不是形式性修改。与上一版相比，代码已经实质关闭了以下高风险问题：

- replay 会重新计算递归失效闭包，删除 `invalidated_refs` 会被识别；
- 已存在的空 snapshot 被判为损坏；
- `(kind, contract_id, revision)` 出现不同 digest 的分叉会被拒绝；
- Store 提交和 replay 会检查 lineage 的 run、turn、完整 Authority Context 连续性；
- Metadata 已绑定 Contract Kind；
- Reference、UUID、Basis Points 已使用受校验反序列化；
- Action metadata 过期会被 Admission 拒绝；
- Response Admission 会重新核对实际 rendered bytes；
- Turn Plan 已增加 DAG、逐 Action 授权 Gate 和模式顺序约束；
- Decision 已增加用户最终选择和权重完整性约束；
- Store 增加跨实例文件锁、锁后重读、snapshot digest chain 和本地 anchor。

但是，IDR 目前仍不是完整、不可绕过的生产 Runtime。根本原因有四个：

1. **生产权威对象仍可通过公开构造器或反序列化进入系统，Rust Runtime 并非唯一合法创建路径。**
2. **Intent、Decision、Action、Response、Human Model 等 Admission 仍主要消费调用方提交的普通 facts、布尔值和普通引用，而不是可信 issuer 签发的 proof。**
3. **Execution claim、permit、dispatch、receipt 和恢复状态机没有闭合，无法区分“尚未执行、正在执行、执行结果未知、已执行但回执丢失”。**
4. **原始设计中的 Orchestration Runtime、不可变 Audit Ledger、Transactional Outbox、Protocol Gateway、Context Fabric 和完整 Outcome Learning 仍未实现。**

项目自身 README 和整改台账也明确保持 `PRODUCTION TRUST ROOT = BLOCKED`，这一结论与本次独立复审一致：

- `README.md:7-10`
- `IDR/README.md:8-23`
- `IDR/docs/audit/IDR_V1_3_REMEDIATION.md:7-13,74-89`

---

# 二、审计范围与验证结果

## 1. 压缩包安全性和完整性

- ZIP 条目：449
- 排除目录、AppleDouble 和 macOS 元数据后的有效文件：327
- 路径穿越：0
- 符号链接：0
- ZIP SHA-256：`87983326ccbdcb59d286e2bb363dd8f6f3231637cb6a673cf1ab5585315e8b4a`
- 包内未提供根级 `SHA256SUMS` 或外部签名，因此只能确认本次读取到的 ZIP 自身摘要，不能证明发布者身份或传输链真实性。

## 2. 执行测试

| 验证 | 结果 | 说明 |
| --- | --- | --- |
| TypeScript Node tests | **6/6 PASS** | `npm test` 可直接运行 |
| Python unittest | **5/5 PASS** | 标准库测试全部通过 |
| TypeScript typecheck | **未完成独立验收** | 包按清单排除了 `node_modules`；`npm ci` 在当前离线环境超时；全局 TypeScript 5.8.3 因缺少 `@types/node` 失败，不能据此判定源码类型错误 |
| Rust workspace tests | **未执行** | 当前审计环境没有 `cargo`/`rustc` |
| Rust Clippy/fmt | **未执行** | 同上 |
| Aegis adapter Rust tests | **未执行** | 同上 |

因此，本报告对 Rust 的结论来自完整静态源码、测试源码、跨对象路径和可构造攻击路径审查，不能把仓库中保存的历史测试输出当成本次独立运行证明。

独立测试输出见：`/mnt/data/idr-v13-reaudit-test-output.txt`。

---

# 三、上一轮问题整改状态

| 上轮问题 | 本轮结论 |
| --- | --- |
| Guard facts 可伪造 | **部分修复，仍为 P0**。增加矛盾 facts 检查，但没有 provenance proof |
| Canonical Input 无来源证明 | **未修复，P0** |
| Authorization 无签名与撤销 | **未修复，P0**；精确字段绑定已加强 |
| Response 只校验摘要格式 | **部分修复，仍为 P0**；实际 bytes 已复核，但 Policy proof 和可信时钟缺失 |
| Human Model 可直接晋级 | **表面部分修复，实际仍可从 wire 绕过，P0** |
| 删除 invalidation 标记恢复旧合约 | **已修复** |
| 空文件/旧 snapshot 回滚 | 空文件已修复；单文件回滚已增强；**snapshot+anchor 同时回滚仍未解决，P0** |
| 同 revision 分叉 | **已修复** |
| Claim 被误报为 AlreadyExecuted | 结果类型已改成 `ExecutionPending`；**但永久悬挂和未知执行状态仍未解决，P0** |
| Receipt 无 permit/provider proof | **未修复，P0** |
| successor 跨上下文 | **Store commit/replay 边界已修复**；Protocol 构造阶段仍不封闭 |
| Medium/Low 不可逆 Action 跳过 Decision | Medium 不可逆已修复；**Low 不可逆仍可跳过，P1** |
| Turn dependency 环和 Gate 指向错误对象 | **已修复** |
| Decision 未记录最终选择 | **已修复** |
| Criteria weights 不完整 | **已修复** |
| Payload 无预算 | 关键 JSON payload 已增加预算，**大部分修复** |
| Aegis 反向耦合 | 没有发现 IDR 核心依赖 Aegis，**核心问题已修复/不存在**；权限衰减仍缺失 |

---

# 四、原始设计与实现对应关系

原始设计将 IDR 定义为四个平面，并明确 Orchestration Runtime 管理 Run/Turn/Step、模块顺序、等待、重试、取消、中断恢复、失效重算和 Turn Plan 执行：

- `source-design/IDR-V1.3-original-design.txt:15-51`
- `source-design/IDR-V1.3-original-design.txt:58-69`

| 原始设计能力 | 当前实现 | 结论 |
| --- | --- | --- |
| Protocol Gateway | 只有 `CanonicalInputEventV1` 数据结构和 Actor–Role 矩阵 | **缺失真实 Gateway、来源认证和 replay index** |
| Context Fabric | 只有 `AuthorityContextV1` 值对象 | **缺失 Context Snapshot、证据、时效和权威数据获取** |
| Intent Fast Path | 纯 Rust Guard 已实现 | **规则局部正确，但 facts 无证明、结果未成为后继强制依赖** |
| Base/Personalized Intent | Intent Contract 已实现，Base Intent 保留约束存在 | **局部实现** |
| Decision Necessity | 纯 Rust Guard 已实现 | **局部实现，facts 无证明** |
| Decision Runtime | Decision Contract 结构已实现 | **没有模型 port、受控解码、证据获取和决策执行 Runtime** |
| Turn Coordination | Selector 与 Plan Contract 已实现 | **局部实现；Assessment 与 Plan 存在跨层不可表示状态** |
| Response Contract/Admission | Contract、Rendered Envelope、Admission 函数存在 | **部分实现；没有签名 Policy proof 和唯一发送 façade** |
| Action Contract/Admission | Contract、Exact Authorization、Admission 和 Store claim 存在 | **部分实现；Authority/Capability/Policy 仍是普通 facts** |
| Execution Runtime | Receipt 类型和 execution claim 存在 | **缺失 dispatch permit、executor enforcement、重试和恢复 Runtime** |
| Outcome Learning | Outcome Record 类型存在 | **缺失 Observation Attestation、调度、归因审查** |
| Human Model Runtime | Assertion 和 Gate 函数存在 | **缺失 Candidate Contract、Promotion Proof、查询/纠正/删除 Runtime** |
| Orchestration Runtime | 当前只有一次性 `assess()` | **架构级缺失** |
| Audit Ledger | 单节点 JSON snapshot + 本地 anchor | **不是原始设计要求的不可变账本和 Transactional Outbox** |
| Capability Registry | Admission facts 中只有 bool | **缺失真实 Registry 和 capability proof** |
| Host Adapter | Aegis shadow adapter 存在 | **边界基本单向，但无权限衰减证明** |

原始设计要求 Execution Receipt 记录 `authorization_ref`，并回答“由谁授权”：

- `source-design/IDR-V1.3-original-design.txt:911-937`

当前 Receipt 没有 authorization、admission、permit 或 dispatch proof，因而尚未达到原始设计。

---

# 五、语言职责边界审核

## Rust

Rust 仍是唯一包含 Guard、Contract、Admission、Store 和 Host Adapter 核心判断的语言。没有发现 TypeScript 或 Python 能直接调用外部执行器或写入 Store。

但“Rust 是生产信任根”目前只能理解为目标边界，而不是已经成立的事实。Rust 内部仍接受普通 facts、普通 JSON 和可反序列化 authoritative contract，且没有唯一 Runtime façade。

## TypeScript

TypeScript 主要进行：

- Canonical Input 形状解析；
- Runtime Assessment 解析；
- Exact Action Authorization Submission 构造。

没有复制 Fast Path、Decision Necessity、Action Admission 或 Human Model Gate 的裁决算法。因此职责边界总体正确。

风险是 TypeScript 手工复制了枚举、Actor–Role 矩阵和 wire 约束，没有由同一个 schema 自动生成，存在长期漂移。

## Python

Python 只加载 fixture、执行离线比较和安全回归，没有生产写入或授权能力，职责边界正确。

但 Python 的 `CanonicalInputObservation` 只是 Canonical Input 的局部投影，不是完整跨语言协议实现，不能作为“Rust/Python schema 完全一致”的证据。

## Aegis Life

没有发现 IDR 核心 crate 引用 Aegis Life。Aegis 只有 `idr-aegis-adapter` 依赖 IDR，方向正确。

`map_authority_context` 只是原样复制 Aegis subject/actor/tenant/scope/purpose/operation，没有签名验证、权限衰减或 host grant → IDR grant 的证明转换。当前仅用于 shadow assessment，因此尚未造成真实执行权限泄漏；一旦用于生产 Admission，该映射不能直接视为可信 Authority。

---

# 六、已正确实现的部分

1. **Contract Kind 已进入 Metadata。**  
   `common.rs:177-192`；各 Contract 校验自身 kind。

2. **受约束 newtype 的 wire 校验明显增强。**  
   Reference、UUID、Basis Points 不再只依赖构造器。

3. **Base Intent 不可被个性化删除。**  
   `intent.rs:226-248`。

4. **Fast Path 条件本身仍保持保守。**  
   低影响、可逆、无歧义、参数完整、不依赖 Human Model 才能 Allow。

5. **Decision 的阶段约束、用户选择和权重约束已加强。**

6. **Action 对 High/Critical 和 Medium 不可逆操作要求 Decision。**  
   `action.rs:130-136`。

7. **Exact Authorization 的对象绑定较精确。**  
   绑定 Action ref、参数摘要、actor、scope、Authority Context digest 和时间窗口：`action.rs:197-268`。

8. **Action metadata 到期会独立拒绝。**  
   `action.rs:321-323`。

9. **Turn Plan 已检查 DAG、Gate 目标和模式顺序路径。**  
   `turn.rs:143-214`。

10. **Response Admission 重新核对实际 rendered bytes。**  
    `response.rs:198-257,304-340`。

11. **Execution Receipt、Outcome、Human Model 在类型上没有混为同一个对象。**

12. **Store 正常提交路径会验证当前指针、依赖新鲜度、跨对象上下文和递归失效。**  
    `idr-store/src/lib.rs:140-218`。

13. **Store replay 会重算 history、current pointer 和 invalidation closure。**

14. **同 revision 分叉和跨上下文 successor 会在 Store 边界拒绝。**

15. **文件锁、锁后重读、临时文件、fsync、rename 和目录 fsync 已实现。**  
    `idr-store/src/lib.rs:795-815,941-955`。

16. **Aegis adapter 是窄的 shadow adapter，没有反向进入 IDR 核心。**

---

# 七、缺失的设计能力

1. Proof-carrying `InputAdmission`、`IntentAdmission`、`DecisionAdmission`、`ResponseAdmissionToken`、`ActionAdmissionPermit`。
2. 可信 issuer key registry、用途绑定、tenant/scope 限制、key rotation 和 revocation。
3. 可信时间源和抗回拨时间证明。
4. Protocol Gateway 的 channel/session/authentication binding 和 nonce replay 防护。
5. Context Snapshot Contract 及其 evidence、freshness、authority provenance。
6. 完整 Orchestration Runtime：持久 Run/Turn/Step、等待、恢复、取消、超时、重试、并行、局部重算。
7. Executor-enforced Execution Permit 和 dispatch proof。
8. Claim 的 lease/owner/attempt/status/recovery 状态机。
9. Outcome Observation Attestation 和 Attribution Review。
10. Human Model Update Candidate Contract、Promotion Decision、用户确认证明和 Outcome 证明。
11. 不可变 Audit Ledger、hash-chained event log、Transactional Outbox。
12. Capability Registry 与 Authority Runtime 的真实接入。
13. Rust/TypeScript/Python 单一 schema 源、canonical bytes 和签名测试向量。
14. Aegis Authority 的验证和单调权限衰减。

---

# 八、P0 问题

## P0-01：权威 Contract 的创建和持久化路径仍未封闭

### 证据

下列权威对象同时派生 `Deserialize`，构造函数又是公开的：

- `intent.rs:146-175`
- `decision.rs:127-151`
- `response.rs:48-68`
- `action.rs:23-44`
- `execution.rs:21-45`
- `outcome.rs:29-50`
- `human_model.rs:147-178`

Store 的公开枚举也可反序列化，`commit()` 接受调用方提供的完整权威 Contract：

- `idr-store/src/lib.rs:28-39`
- `idr-store/src/lib.rs:140-145`

### 可复现方式

1. 根据公开 schema 构造一份 Contract JSON。
2. 按公开 digest 算法计算 `record_digest`。
3. 通过 Serde 反序列化为 `HumanCenteredContractSnapshotV1`。
4. 调用 `store.commit()`。
5. 只要局部 shape、依赖和 Store 当前性检查通过，Store 无法区分它是 Runtime 签发还是外部伪造。

### 影响

Rust 虽然负责校验，但没有成为**唯一颁发者**。外部调用者仍可制造权威状态，而不只是提交候选。

### 推荐修复

- 权威 Contract 类型移除公共 `Deserialize`；
- `issue()` 改为 `pub(crate)` 或放入 sealed trust-root crate；
- wire 层只允许 `CandidateV1`/`SubmissionV1`；
- Store 只接受 Runtime 内部签发的 `CommittedContract<T>` 或带 MAC/签名的 opaque commit token；
- 对历史 replay 使用独立、严格的 `StoredContractEnvelopeV1`，而不是复用外部 wire DTO。

---

## P0-02：Intent Fast Path、Decision Necessity 和 Turn Coordination 仍消费无来源 facts

### 证据

`InteractionAssessmentRequestV1` 直接包含三组可反序列化 facts：

- `idr-runtime/src/lib.rs:15-22`

Runtime 只执行局部矛盾检查，再调用纯函数：

- `idr-runtime/src/lib.rs:48-60,99-117`

facts 没有：

- 输入事件摘要；
- Context Snapshot ref；
- evidence refs；
- producer/issuer；
- policy revision proof；
- 时间窗口；
- 签名。

Intent Contract 也不保存 Fast Path Admission ref：

- `intent.rs:146-159,212-254`

### 可复现方式

调用方提交：

```text
deterministic_command_match=true
required_parameters_complete=true
ambiguity_present=false
authority_context_valid=true
policy_allows_request=true
impact_level=low
reversible=true
depends_on_human_model=false
```

Runtime 无法证明这些事实来自 Command Registry、Policy Engine 或 Context Fabric。

### 推荐修复

定义并持久化：

```text
IntentFastPathFactsProofV1
DecisionNecessityFactsProofV1
TurnCoordinationFactsProofV1
```

至少绑定 input refs、context snapshot、evidence digest、producer、rule/policy version、issued/expiry 和签名。后继 Contract 必须把相应 Admission ref 纳入 dependency。

---

## P0-03：Canonical Input 只有角色矩阵，没有可信来源证明

### 证据

Canonical Input 保存 `source_actor` 和 `actor_ref`，但对象本身可反序列化：

- `input.rs:41-56`

校验只检查 shape、摘要格式和 Actor–Role 矩阵：

- `input.rs:92-117,149-165`

没有 Gateway signature、authenticated channel、session binding、nonce、ingress sequence 或 replay index。

### 可复现方式

构造一个 shape 合法的 JSON：

```text
source_actor = user
actor_ref = actor:user-001
semantic_roles = [intent, command]
```

只要 UUID、引用和摘要格式正确，Rust 无法区分它来自真实用户通道还是伪造客户端。

### 推荐修复

新增 `CanonicalInputAdmissionV1`，由 Protocol Gateway 在验证 session/channel/device/authentication 后签发，并绑定 event ID、content digest、actor、tenant、nonce、received_at 和 channel context。

---

## P0-04：Action Authorization 和 Action Admission 没有可信 Authority/Capability/Policy 证明

### 证据

Exact Authorization 的字段绑定已加强，但没有：

- issuer；
- key ID；
- signature/MAC；
- authentication context；
- revocation status；
- key rotation；
- trusted-clock proof。

见：`action.rs:197-268`。

Action Admission 继续消费调用方布尔值：

- `action.rs:279-292,312-379`

Store 的 `claim_action_execution()` 仍把这些 facts 直接传给 Admission：

- `idr-store/src/lib.rs:221-230`

Store 不会自行查询 Capability Registry、Authority Runtime、Policy Engine 或业务前置条件。

### 可复现方式

1. 对一个 current Action 构造字段一致的 Authorization JSON。
2. 设置 `capability_registered=true`、`actor_authority_valid=true`、`policy_allows=true` 等全部 facts 为 true。
3. 传入调用方选择的 `valid_at`。
4. Store 可以生成 execution claim。

### 推荐修复

实现签名的：

```text
AuthorityGrantProofV1
CapabilityProofV1
PolicyEvaluationProofV1
PreconditionSnapshotProofV1
DependencySnapshotProofV1
```

Admission 只能消费验证后的 proof newtype；`valid_at` 必须来自 trust-root clock；Authorization 必须在 claim 事务中校验未撤销并原子消费。

---

## P0-05：Response Admission 仍可伪造 Policy facts，并可使用调用方时间重放

### 证据

本轮已经正确增加实际 rendered bytes 校验：

- `response.rs:198-257,304-315`

但 `ResponsePolicyFactsV1` 仍是可反序列化普通对象，其中政策结果是裸布尔值：

- `response.rs:272-286`

`valid_at` 由调用方传入：

- `response.rs:304-310,335-340`

`ResponseAdmissionDecisionV1` 没有签名、内容发送 nonce、单次消费状态或持久发送记录。

### 可复现方式

1. 对真实 rendered content 计算合法 envelope。
2. 自行构造绑定一致的 policy refs/digest/time。
3. 将五个 policy bool 全部设为 true。
4. 传入仍位于旧 policy window 的 `valid_at`。
5. 函数返回 `Allow`，但没有证据证明 Policy Engine 实际作出该决定。

### 推荐修复

- Policy Engine 签发 `PolicyEvaluationProofV1`；
- Response Runtime 使用可信当前时间；
- 生成与 response/content/policy/recipient/channel 精确绑定的单次 `ResponseSendPermitV1`；
- 唯一发送 façade 消费 permit 并记录 send receipt；
- 产品层不能直接发送未经 permit 的内容。

---

## P0-06：Human Model 仍能绕过 Gate，从 wire 直接晋级

### 证据

`HumanModelAssertionV1::issue()` 确实要求 `HumanModelUpdateGateDecisionV1`：

- `human_model.rs:176-203`

但 Assertion 本身仍派生 `Deserialize`，并直接持久化 `epistemic_status`：

- `human_model.rs:147-173`

`validate()` 不要求 Gate Decision、Candidate、用户确认事件或 Outcome ref：

- `human_model.rs:269-306`

Store 对 Human Model 没有任何跨对象检查：

- `idr-store/src/lib.rs:790`

Gate facts 仍是普通布尔值和计数：

- `human_model.rs:324-334,392-448`

虽然 Contract Kind 枚举包含 `HumanModelUpdateCandidate`，代码中没有对应 Contract 类型和 Store 分支：

- `common.rs:63-75`

### 可复现方式

1. 构造 `HumanModelAssertionV1` JSON。
2. 直接填入 `epistemic_status=user_confirmed` 或 `outcome_supported`。
3. 使 subject 与 Authority Context subject 相同。
4. 重新计算 record digest。
5. 反序列化并提交 Store。

不需要 Gate Decision，也不需要用户确认事件或 Outcome Record。

### 推荐修复

- 移除 Assertion 的外部 `Deserialize`；
- 实现独立 `HumanModelUpdateCandidateV1`；
- `PromotionDecisionV1` 必须绑定 candidate、subject、predicate/value digest、用户事件或 Outcome refs；
- Store 必须验证 Candidate → Promotion → Assertion 链；
- UserConfirmed 必须引用经过 Gateway Admission 的用户确认；
- OutcomeSupported 必须引用可信 Outcome Observation Attestation。

---

## P0-07：Execution claim 没有生命周期，崩溃后会永久 `ExecutionPending`

### 证据

Claim 只保存：

- authorization ID；
- action ref；
- tenant；
- operation；
- idempotency key；
- claimed_at。

见：`idr-store/src/lib.rs:106-115`。

已有 claim 时永久返回 `ExecutionPending`：

- `idr-store/src/lib.rs:271-283`

没有 owner、lease、renew、attempt、dispatch state、heartbeat、uncertain、reconcile、release 或 recovery API。

### 可复现方式

1. `claim_action_execution()` 成功持久化 claim。
2. 进程在调用外部 Provider 前崩溃。
3. 重启后再次 claim。
4. 永久得到 `ExecutionPending`。

系统无法判断：

- 从未执行；
- 已经调用但结果未知；
- 已成功但 Receipt 丢失。

### 推荐修复

将 claim 改为持久执行状态机：

```text
RESERVED → DISPATCHED → ACKNOWLEDGED → VERIFIED
                  ↘ UNKNOWN → RECONCILING → RETRYABLE/VERIFIED
                  ↘ FAILED → RETRYABLE/TERMINAL
```

每个 permit 包含 owner、attempt、lease_until、provider、authorization、action digest 和 nonce。恢复只能在 Provider reconciliation 后决定重试。

---

## P0-08：Execution Receipt 没有绑定 Execution Permit、Authorization 或 Provider 签名

### 证据

Receipt 字段没有：

- authorization ref；
- Action Admission ref；
- execution permit ID；
- dispatch proof；
- provider signature；
- capability proof。

见：`execution.rs:21-41`。

这与原始设计中 Receipt 的 `authorization_ref` 要求不一致：

- `source-design/IDR-V1.3-original-design.txt:911-937`

Store 仅检查存在一个相同 action 且 `claimed_at <= started_at` 的 claim：

- `idr-store/src/lib.rs:715-736`

### 可复现方式

1. 为 Action 取得任意合法 claim。
2. 自行构造 Receipt，provider 使用任意非空 Reference。
3. 填写 started_at 不早于 claim。
4. 计算合法 digest。
5. Store 无法证明它由真实执行器、真实 dispatch 或真实 Provider 产生。

### 推荐修复

- Store 原子签发不可伪造 `ExecutionPermitV1`；
- Executor 只接受 Permit；
- Receipt 必须绑定 permit ID、admission ref、authorization ref、attempt、dispatch nonce；
- Provider 或受信 executor 对 Receipt 签名；
- Store 验证 permit 当前状态和签名后才能持久化 Receipt。

---

## P0-09：Store 的 anti-rollback 仍是本地双文件方案，不是可信不可变账本

### 证据

源码明确声明当前 Store 不是生产不可变 Audit Ledger：

- `idr-store/src/lib.rs:1-7`
- `IDR/README.md:19-23`

Snapshot 和 anchor 位于同一信任域。若攻击者同时回滚两者，校验会接受一致的旧状态。特别是旧 sequence-1 snapshot 配合删除 anchor，会被当作首次 anchor 恢复：

- `idr-store/src/lib.rs:838-868`

原始设计要求 append-only hash-chain Audit Ledger 和 Transactional Outbox：

- `source-design/IDR-V1.3-original-design.txt:1043-1087`

### 可复现方式

1. 保存 sequence 1 的 snapshot 副本。
2. Store 运行到更高 sequence。
3. 用 sequence 1 覆盖 snapshot，并删除/回滚 `.anchor`。
4. 打开 Store；sequence 1 且无 anchor 的路径会重新 append anchor。

### 推荐修复

- 生产层使用 PostgreSQL append-only audit events + aggregate version CAS；
- Transactional Outbox 与业务状态同事务提交；
- hash chain root 定期外部锚定到独立 WORM/Object Lock/远程透明日志；
- 本地 snapshot 只作为缓存或开发后端，不作为生产信任根。

---

## P0-10：原始设计中的 Orchestration Runtime 尚未实现

### 证据

原始设计要求：

- Run/Turn/Step 状态；
- 模块顺序；
- 等待输入/授权/依赖；
- 并行、超时、重试、取消；
- 中断与恢复；
- 失效传播和局部重算；
- 执行 Turn Coordination Plan。

见：`source-design/IDR-V1.3-original-design.txt:58-69,1010-1041`。

当前 `idr-runtime` 只有一次性 `assess()`：

- `idr-runtime/src/lib.rs:44-97`

`InteractionRunStateV1` 只是状态枚举和 transition validator，没有持久 Run/Step 实体或事件处理循环。

### 可复现方式

代码库中无法找到：

- create/resume/cancel run；
- step scheduler；
- timeout/retry policy；
- wait handle；
- interruption recovery；
- Turn Plan executor；
- state event log。

### 推荐修复

先实现最小持久 Orchestrator：

```text
RunRecord
TurnRecord
StepRecord
StateEvent
WaitCondition
RetryPolicy
CancellationToken
ContractDependencySnapshot
```

所有 Contract 颁发、Admission、claim、Receipt 和 Outcome 必须由 Orchestrator 驱动并记录 causation/correlation。

---

## P0-11：Outcome 没有真实 Observation Attestation，可制造虚假结果并污染 Human Model

### 证据

Outcome 是公开可反序列化对象：

- `outcome.rs:29-46`

其 evidence 是普通 Reference，缺少 observer、observed_at、data source、signature、collection policy、freshness 和 attribution reviewer：

- `outcome.rs:107-180`

Store 只检查引用和部分状态一致性，不验证观察真实性：

- `idr-store/src/lib.rs:738-789`

Human Model Gate 又只接受 `outcome_support_present=true`：

- `human_model.rs:422-427`

### 可复现方式

1. 构造一个 `Observed` Outcome，填入任意非空 observed results 和 evidence refs。
2. 关联合法 Decision/Action/Receipt 引用。
3. 计算 digest 并提交 Store。
4. 在 Human Model facts 中设置 `outcome_support_present=true`。

### 推荐修复

实现：

```text
OutcomeObservationAttestationV1
OutcomeAttributionReviewV1
HumanModelUpdateCandidateV1
```

观察证明必须绑定来源、时间、对象、指标、采集政策、原始证据摘要和签名；归因与观察分离，Human Model 只能消费审查后的 Candidate。

---

# 九、P1 问题

## P1-01：Runtime Assessment 可以生成无法表示为合法 Turn Plan 的模式

### 证据

Selector 在 `action_planned=false` 时无条件返回 `RespondOnly`，即使 `response_planned=false`；在有 Action 且无 Response 时返回 `ActThenRespond`：

- `guards.rs:204-224`

Runtime 的 facts validator 没有禁止这些组合：

- `idr-runtime/src/lib.rs:99-117`

Turn Plan 却对所有模式强制 `response_refs` 非空：

- `turn.rs:113-130`

### 可复现方式

```text
response_planned=false
action_planned=false
```

Runtime 返回 `RespondOnly`，但不存在可满足 `response_refs.is_empty() == false` 的计划。

或者：

```text
response_planned=false
action_planned=true
```

Runtime 返回 `ActThenRespond`，语义上又要求一个并未计划的 Response。

### 修复

- 若所有 Turn 都必须有 Response，则 Runtime 明确拒绝 `response_planned=false`；
- 或新增 `NoOp`/`ActOnly` 等正式模式并同步 Contract schema；
- 增加 Assessment → Turn Plan 可实现性 property test。

---

## P1-02：Low impact 不可逆 Action 仍可跳过 Decision

`ActionContractV1::validate()` 只对 Medium 及以上不可逆操作要求 Decision：

- `action.rs:130-136`

而 Decision Necessity Guard 把任何 `irreversible_result=true` 视为需要 Decision。

### 修复

任何不可逆 Action 都应要求：

- Decision Contract；或
- 一个受证明的 `NoDecisionRequiredAdmission`，说明用户选择已明确或只有唯一合法操作。

---

## P1-03：Decision 的用户选择没有绑定到 Action 的具体语义

Decision 保存 `selected_option_ref`，但 Action 只引用整个 Decision。Store 只检查 Authority Boundary，没有检查 Action operation/parameters 对应被选择的 option：

- `decision.rs:127-147`
- `action.rs:25-39`
- `idr-store/src/lib.rs:683-713`

### 影响

用户选择 option B 后，系统仍可能创建一个语义无关但权限边界合法的 Action C。

### 修复

Decision Option 应包含 action template/effect ref，Action 保存 `selected_option_ref` 或 `decision_effect_ref`，Store 比较 operation 和 parameter constraints。

---

## P1-04：失败、拒绝和部分成功后的重试语义不可实现

Receipt 有 `attempt` 字段：

- `execution.rs:31`

但 Store 拒绝同一 Action 的任何第二个 Receipt：

- `idr-store/src/lib.rs:723-729`

已有 claim 又始终返回 `ExecutionPending`，因此 Failed/Rejected/PartiallySucceeded 后没有正式 retry path。

这与原始设计要求的重试、失败恢复和补偿不一致：

- `source-design/IDR-V1.3-original-design.txt:898-909`

### 修复

以 `(action_ref, attempt)` 管理多个 Receipt；明确定义 terminal、retryable、compensating 和 reconciled 状态；每次 attempt 必须使用新 permit。

---

## P1-05：Store anchor 部分写入缺少自动恢复

Anchor 使用 append JSON line：

- `idr-store/src/lib.rs:912-932`

读取时任何空行、半行或损坏 JSON 都会导致失败：

- `idr-store/src/lib.rs:871-909`

没有尾部截断恢复、双写 commit marker 或 WAL。断电发生在 anchor 行写入中间时，Store 可能永久无法打开。

### 修复

使用长度前缀 + checksum + commit marker；启动时只允许截断最后一个未完成 record；增加每个 I/O 边界的 crash injection test。

---

## P1-06：当前 Store 不是原始设计要求的 Audit Ledger

当前保存的是完整 snapshot，不是不可变事件流。它没有：

- event ID/type；
- actor、causation、correlation；
- model/prompt/policy version；
- append-only business event；
- Transactional Outbox。

原始要求见：`source-design/IDR-V1.3-original-design.txt:1043-1087`。

---

## P1-07：跨语言协议仍是手工复制，存在语义漂移

### 证据

- Rust `u64` 可接受完整 64 位范围；TypeScript 只接受 JS safe integer：`index.ts:349-353`。
- TypeScript UUID regex 限制版本/variant；Rust UUID newtype 主要拒绝 nil，接受范围不同：`index.ts:422-434`。
- Python 只投影 source actor、roles、content ref/digest，忽略 event/run/turn/actor/correlation/logical time/schema：`contracts.py:38-79`。
- Rust 使用 `deny_unknown_fields`；TS parser 会提取已知字段但不会拒绝未知字段。
- Actor–Role 矩阵在 Rust、TS、Python 三处手工维护。

### 修复

从语言中立 schema 生成三种语言类型和 validators，增加 full-fixture negative corpus 和每字段边界向量。

---

## P1-08：摘要编码没有冻结 canonical bytes，也不提供真实性

Rust Contract digest 使用 `serde_json::to_vec()`：

- `common.rs:366-375`

Store digest也使用 Serde JSON tuple：

- `idr-store/src/lib.rs:564-578`

尚未定义 JCS/RFC 8785 或等价 canonical encoding；也没有三语言字节级 digest/signature vectors。

SHA-256 只能检测内容变化，不能证明 issuer 身份。

---

## P1-09：Aegis Authority 映射没有验证或权限衰减

`map_authority_context()` 原样复制所有字段：

- `Aegis-Life/crates/idr-aegis-adapter/src/lib.rs:17-35`

没有证明：

- Aegis Context 由谁签发；
- scope 是否被收窄；
- operation 是否允许；
- tenant 是否匹配；
- host grant 是否可转换为 IDR grant。

此外 adapter Cargo 声明了未使用的 `idr-store` 依赖：

- `Aegis-Life/crates/idr-aegis-adapter/Cargo.toml:6-10`

### 修复

Adapter 只能产生 `HostAuthorityCandidateV1`；由 IDR Authority Runtime 验证并签发收窄后的 proof。删除未使用 store 依赖。

---

## P1-10：测试仍未覆盖关键攻击、故障和并发矩阵

新增测试已覆盖 invalidation、rollback 单文件、revision fork、cross-context、并发 claim、Decision boundary 和 Receipt claim，进步明显。

仍缺少：

- authoritative Contract 反序列化伪造；
- Human Model `USER_CONFIRMED` wire bypass；
- Gateway source spoofing；
- Authorization signature/revocation/key rotation；
- trusted-clock rollback；
- Response policy proof伪造和重复发送；
- snapshot+anchor 同时回滚；
- anchor 半行/断电恢复；
- claim lease/recovery/owner takeover；
- 外部执行成功但 Receipt 丢失；
- Failed/Partial receipt 后 retry；
- Provider impersonation；
- Outcome observation forged evidence；
- Runtime→Turn Plan 可实现性；
- Rust/TS/Python canonical byte/digest vectors；
- Aegis privilege widening。

---

# 十、P2 问题

## P2-01：TypeScript 授权构造器不做运行时 decision 校验

`buildExactActionAuthorizationSubmission()` 依赖 TypeScript 静态类型，但对 JS/untyped 调用者不校验 `decision`：

- `packages/interaction-client/src/index.ts:274-287`

应使用 runtime enum validator。

## P2-02：`parseInteractionAssessment()` 只校验字段形状，不校验跨字段不变量

它可以接受：

- Fast Path Allow + Decision Required；
- Waiting Authorization 但 coordination mode 不是 confirm；
- Blocked posture 但 next state 任意。

虽然 TS 不应复制 Rust裁决算法，但至少应验证协议层声明的不变量，或验证 Runtime 响应签名。

## P2-03：Blocked request 被标记为 `Succeeded`，语义容易误用

`idr-runtime/src/lib.rs:62-70` 将 Fast Path Block 返回为：

```text
action_posture=blocked
next_run_state=succeeded
```

如果 `Succeeded` 表示“安全处理完毕”可以成立，但如果上层理解为“用户目标完成”，会形成错误指标。应增加 `CompletedBlocked`/终止原因或明确规范。

## P2-04：Human Model corrected value 的消费语义不明确

Assertion 同时保存原始 `value` 和 `corrected_value`，但没有权威 `effective_value()`；不同消费者可能继续读取旧值。

## P2-05：包没有独立完整性清单或发布签名

本次 ZIP 可安全解压，但根目录没有 `SHA256SUMS`、签名或 provenance attestation。正式审计发布包应提供 detached signature/SLSA provenance，并固定 Cargo/npm/Python 依赖来源。

## P2-06：Store 文件替换和目录 fsync 的平台语义未定义

`fs::rename(temporary, path)` 在不同操作系统对覆盖已存在目标的行为不同；目录 `sync_all()` 也不是所有平台都同等支持。应明确生产支持平台并增加对应测试。

## P2-07：协议版本只有固定常量，没有兼容协商和迁移规则

需要定义 minimum reader/writer version、unknown enum policy、migration、signature domain version 和 deprecation window。

---

# 十一、十项原审计问题的最终答案

1. **设计与实现逐项对应：** 协议对象和局部规则较完整；Control Plane、Gateway、Context、Proof、Execution、Audit、Outcome Learning 仍缺失。
2. **语言职责边界：** TS/Python 没有明显越权；Rust 内部信任边界未封闭。
3. **五类门禁绕过：** 仍存在。核心绕过来自无证明 facts、可反序列化权威对象和非唯一执行/发送入口。
4. **Action 授权精确绑定：** 对 ID、revision、record digest、parameter digest、actor、scope、Authority Context 和有效期绑定较好；真实性、撤销、可信时间和 executor consumption 未完成。
5. **依赖、替换、递归失效：** 单节点 Store 正常提交/replay 逻辑已明显可靠；外部 anti-rollback、不可变账本和多机一致性不足。
6. **Receipt、Outcome、Human Model 混合：** 类型没有混合；但真实证明链没有闭合，Outcome 可伪造并污染 Human Model。
7. **状态持久性、原子性、重放、损坏检测：** 单机 snapshot 基线增强；不是生产 ledger；claim recovery 和断电恢复不足。
8. **跨语言协议和 fixture：** 基础 fixture 可共享；schema、整数、UUID、unknown fields、canonical encoding 和 digest vectors仍漂移。
9. **Aegis 反向耦合/权限泄漏：** 未发现核心反向依赖；当前 shadow adapter 不执行，但 Authority 只是直拷贝，没有权限衰减和真实性。
10. **测试覆盖：** 正常路径和部分攻击路径明显增加；签名、撤销、崩溃、未知执行状态、证明链和跨语言字节一致性仍不足。

---

# 十二、下一阶段实施顺序

## 阶段 0：保持发布阻断

```text
PRODUCTION_RELEASE_AUTHORIZED = false
PRODUCTION_AUTHORIZATION_ENABLED = false
PRODUCTION_EXECUTION_ENABLED = false
LONG_TERM_HUMAN_MODEL_WRITE_ENABLED = false
AEGIS_PRODUCTION_ADAPTER_ENABLED = false
```

## 阶段 1：封闭权威对象创建路径

1. 移除权威 Contract 的外部 Deserialize。
2. 所有 authoritative `issue()` 改为 trust-root 内部可见。
3. Store 只接受 sealed Runtime output。
4. 分离 Candidate DTO、Stored Envelope 和 Authoritative Contract。

## 阶段 2：建立统一 Proof Framework

实现并签名：

```text
InputAdmissionProof
ContextSnapshotProof
AuthorityGrantProof
PolicyEvaluationProof
CapabilityProof
HumanConfirmationProof
DependencySnapshotProof
```

建立 issuer registry、purpose、tenant/scope、rotation、revocation 和 trusted clock。

## 阶段 3：实现真正的 Orchestration Runtime

持久化 Run/Turn/Step、命令、状态事件、Wait、Retry、Timeout、Cancel、Resume、Invalidation 和 Turn Plan execution。

## 阶段 4：闭合 Action → Execution

```text
ActionAdmission
→ Atomic Execution Permit
→ Dispatch
→ Provider Acknowledgement
→ Receipt
→ Verification/Compensation
```

增加 claim lease、owner、attempt、unknown/reconcile/retry 状态。

## 阶段 5：闭合 Outcome → Human Model

```text
Observation Attestation
→ Outcome Record
→ Attribution Review
→ Human Model Update Candidate
→ Promotion Decision
→ Assertion
```

禁止 Outcome 和 Gate 使用裸 bool 作为长期学习依据。

## 阶段 6：替换生产 Store

使用 append-only Audit Ledger、aggregate CAS、Transactional Outbox、外部 hash root anchor；当前 JSON Store保留为单机开发/测试 backend。

## 阶段 7：冻结跨语言规范

- 单一 schema source；
- JCS/等价 canonical encoding；
- digest/signature golden vectors；
- full positive/negative fixtures；
- Rust/TS/Python CI 字节级一致性。

## 阶段 8：生产 Aegis Adapter

Adapter 只提交 candidate；Authority 必须验证并衰减；Execution 只能消费 IDR permit；加入 privilege monotonicity tests。

## 阶段 9：独立最终验收

最低发布门槛：

```text
P0 = 0
P1 security/reliability = 0
Rust test/clippy/fmt = PASS
TypeScript test/typecheck = PASS
Python release gates = PASS
Crash matrix = PASS
Authorization revocation/replay = PASS
Execution unknown-state recovery = PASS
Cross-language canonical vectors = PASS
Aegis privilege monotonicity = PASS
```

---

# 十三、最终评价

当前 IDR V1.3 已经从“协议类型原型”进化为一个**结构约束较强、具备单节点重放和失效治理能力的 foundation kernel**。本轮整改质量总体是可信的，尤其是 Store 重放、revision fork、Contract Kind、Turn DAG、Decision stage 和实际 Response bytes 校验。

但它尚未从“校验调用方声明”跨越到“验证可信事实并控制唯一执行路径”。在完成签名 proof、封闭构造路径、Orchestration Runtime、Execution Permit、Outcome Observation 和生产 Audit Ledger 之前，IDR 不能承担生产授权、执行和长期 Human Model 的信任根职责。
