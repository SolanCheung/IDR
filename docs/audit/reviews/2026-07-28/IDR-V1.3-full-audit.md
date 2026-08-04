# The Human-Centered Intent & Decision Runtime（IDR）V1.3 完整复审报告

审计日期：2026-07-28  
审计对象：`IDR-V1.3-2026-07-28.zip`  
系统正式名称：**The Human-Centered Intent & Decision Runtime**  
简称：**IDR**

## 1. 总体结论

**最终结论：`PRODUCTION TRUST ROOT = BLOCKED`。**

本审计包比上一轮零散附件完整得多，已经包含原始设计、整改前审计、Rust protocol/runtime/store、TypeScript interaction client、Python evaluation、共享 fixture 与 Aegis Life adapter。包内 README 与整改台账也主动保持了 `BLOCKED` 结论，这一点是正确的。

当前实现已经从“纯协议类型集合”推进到“具备局部持久化和跨对象校验的 foundation/shadow-mode 内核”，但仍不是不可绕过、可证明、可恢复的生产 Runtime。决定性原因有四类：

1. **Admission 仍主要消费调用方可伪造的 facts/布尔值，而不是 Rust 验证过的 proof。**
2. **Authorization、Policy Evaluation、Human Confirmation、Capability 和 Provider Receipt 没有可信 issuer、签名、撤销和用途绑定。**
3. **Store 仍是可替换 JSON snapshot，不是设计要求的不可变审计账本；失效状态、回滚和空文件损坏存在实质绕过。**
4. **Execution claim 不是可租约、可恢复、不可伪造的 permit，崩溃后会造成永久误判，Receipt 也没有绑定 permit。**

因此，Rust 虽然是代码上的主要裁决层，但尚未成为真正的“生产信任根”。

---

## 2. 审计范围与独立验证

### 2.1 解压与完整性

- ZIP 总条目：179。
- 排除目录项、`__MACOSX` 和 AppleDouble 后，有效文件：52。
- 有效内容约 427 KB、13,634 行文本。
- 未发现路径穿越或符号链接攻击。
- `SHA256SUMS` 对 52 个文件全部验证通过。

### 2.2 实际执行结果

- TypeScript 运行测试：**6/6 通过**。
- Python 评估测试：**5/5 通过**。
- TypeScript typecheck：**未能独立完成**。审计包按说明排除了 `node_modules`；本环境无法在时限内重新安装 `@types/node`，现有 `tsc --noEmit` 因缺少 Node 类型定义失败，不是已证实的源码类型错误。
- Rust：本环境没有 `cargo`/`rustc`，因此无法独立执行 `cargo test`、`clippy`、`fmt`。Rust 结论来自完整静态代码审查、测试代码审查和可构造攻击路径分析，不能把仓库自述的本地测试结果当作本次独立执行证据。

测试输出见同目录 `idr-audit-test-output.txt`。

---

## 3. 原始设计与实现对应关系

| 原始设计能力 | 当前实现 | 结论 |
| --- | --- | --- |
| Protocol Gateway | 只有 `CanonicalInputEventV1` 和 TS/Python shape validation，没有认证、签名和 replay-safe Gateway | 缺失生产能力 |
| Context Fabric | Metadata/Authority Context 可携带引用，但没有 Context Snapshot、provenance、freshness proof | 缺失 |
| Intent Fast Path | 有纯 Rust guard | 局部实现；可绕过/可伪造 facts |
| Intent Resolution | 有 Intent Contract 及基础意图保留约束 | 合约层实现；Provider、structured decode 和 admission proof 缺失 |
| Decision Necessity | 有纯 Rust guard | 局部实现；未成为不可绕过前置证明 |
| Decision Runtime | 有 Decision Contract | 没有完整状态式决策运行时和证据采集流程 |
| Turn Coordination | 有 selector 和 Turn Contract 的 DAG/顺序校验 | 局部正确；selector 结果未绑定计划 |
| Response Admission | 有渲染摘要和 policy binding 字段 | proof/issuer/当前时刻发送门缺失 |
| Action Admission | 有 exact authorization、facts gate、durable claim | proof、可信时间、permit、撤销和恢复缺失 |
| Execution Runtime | 只有 claim + Receipt contract | 没有唯一执行入口、租约状态机和执行 permit |
| Outcome Runtime | 有 Outcome Record 和跨对象链接 | 没有 observation attestation/调度/因果复核 |
| Human Model Update Gate | gate decision 不再可反序列化，Assertion 必须消费 decision | facts 仍可伪造，candidate/evidence/subject 未绑定 |
| Orchestration Runtime | `idr-runtime` 只做一次 assessment | 与设计要求的中断、恢复、取消、重试、超时、版本失效、并行控制差距很大 |
| Immutable Audit Ledger | JSON snapshot + rename/fsync | 不符合不可变日志、hash chain、outbox 设计 |
| Policy/Capability Registry | 只有普通引用和布尔 facts | 缺失可信 registry 和 proof |
| Rust/TS/Python 边界 | TS 只解析/提交，Python 只离线评估 | 当前提供代码没有明显越界，但生产入口未封闭 |
| Aegis Life reference host | adapter 单向依赖 IDR，核心不依赖 Aegis | 基本正确；权限衰减和 API 暴露仍不足 |

原始设计要求 Orchestration Runtime 管理 run/turn/step、等待、并行、超时、重试、取消、中断恢复、失效重算以及 Turn Plan 执行；当前 runtime 只调用三个纯函数并返回一次 assessment。证据：`source-design/IDR-V1.3-original-design.txt:58-69,1010-1041`；`IDR/crates/idr-runtime/src/lib.rs:15-90`。

---

## 4. 已正确实现的部分

1. **Contract Metadata 已绑定 contract kind。**  `initial` 和 `successor` 都要求 kind，successor 禁止跨 kind：`IDR/crates/idr-protocol/src/human_centered/common.rs:176-253,287-317`。
2. **受约束 newtype 已采用校验式反序列化。** Reference、UUID ID、Basis Points 已修复上一轮显式 Serde 绕过问题：`IDR/crates/idr-protocol/src/lib.rs:7-39`；`.../common.rs:12-104`。
3. **Fast Path 的局部规则是保守的。** 低影响、可逆、参数完整、无歧义、不依赖 Human Model 才允许：`.../guards.rs:6-87`。
4. **Decision Unknown 会保守进入 Decision Runtime。** `.../guards.rs:91-177`。
5. **Base Intent 不允许被个性化删除。** `.../intent.rs:212-254`。
6. **高影响 Action 强制 Decision 和授权状态。** `.../action.rs:128-132`。
7. **Exact Authorization 已精确绑定 Action ref、参数摘要、actor、scope 和 Authority Context digest。** `.../action.rs:193-264`。
8. **Action metadata 过期会在 Admission 中拒绝。** `.../action.rs:308-319`。
9. **Turn Plan 已增加 DAG、逐 Action 授权 gate 和模式顺序约束。** `.../turn.rs:113-219`。
10. **Rendered Response 构造时由 Rust 从真实内容计算摘要。** `.../response.rs:188-220`。
11. **Response Policy facts 已绑定 response ref、内容摘要、policy revision 和评估时间。** `.../response.rs:255-313`。
12. **Human Model Assertion 不再由调用方直接传 epistemic status。** 必须消费内部构造的 gate decision：`.../human_model.rs:176-235,347-389`。
13. **Canonical Input 增加了 actor-role 矩阵。** Rust、TS、Python 当前矩阵一致：`.../input.rs:92-165`；`packages/interaction-client/src/index.ts:379-420`；`research/evaluation/src/idr_eval/contracts.py:23-35`。
14. **Store commit 在本机多实例场景使用文件锁、锁后重读、临时文件、文件 fsync、rename 和目录 fsync。** `idr-store/src/lib.rs:123-190,619-668`。
15. **Store 会在正常 commit 路径递归标记依赖旧修订的 current contracts 为失效。** `idr-store/src/lib.rs:316-348`。
16. **Store 已对 Decision→Intent、Action→Decision、Receipt→Action、Outcome→Receipt/Action 的同上下文关系进行复核。** `idr-store/src/lib.rs:474-613`。
17. **Aegis 核心没有反向进入 IDR。** adapter 只从 Aegis 映射引用，IDR crates 没有依赖 Aegis：`Aegis-Life/crates/idr-aegis-adapter/src/lib.rs:8-35`。
18. **TypeScript 当前只解析 Rust 输出和构造授权提交，没有复制 Fast Path/Decision 规则。** `packages/interaction-client/src/index.ts:181-287`。
19. **Python 当前代码只做离线评估。** `research/evaluation/src/idr_eval/evaluator.py:10-64`。

这些整改是实质性的，但主要强化的是“对象形状和正常路径”，尚未闭合“证明来源、唯一入口和持久恢复”。

---

## 5. 缺失的设计能力

1. 签名并可撤销的 `AuthorityGrantProofV1`。
2. 签名并绑定内容的 `PolicyEvaluationProofV1`。
3. `CapabilityProofV1` 与 Capability Registry snapshot。
4. `DependencySnapshotProofV1` 与 authoritative store sequence/root hash。
5. `HumanConfirmationProofV1`、`OutcomeObservationProofV1`。
6. Gate Decision 对 facts digest、input refs、context snapshot、policy revision、目标 contract 的精确绑定。
7. 可信时间源或受证明的 logical time；生产 API 不应接收调用方传入的 `valid_at`。
8. 不可伪造、可租约、可续租、可取消、可完成、可恢复的 Execution Permit。
9. Receipt 对 Authorization、Admission Decision、Permit、Provider identity/signature 的绑定。
10. Outcome Observation 调度、证据抓取、归因审查与签名。
11. Human Model Candidate 与 Promotion Decision 的独立对象和长期审计链。
12. append-only event log、hash chain/Merkle root、anti-rollback anchor、transactional outbox。
13. 完整 Orchestration Runtime 状态机及其 durable transitions。
14. 语言中立 schema、canonical bytes、digest/signature golden vectors。
15. Aegis Authority 的衰减/收窄证明，而不是字符串复制。

---

# 6. P0 问题

## P0-01：Intent Fast Path、Decision Necessity 和 Turn Coordination 仍由调用方 facts 驱动

**证据**

- `InteractionAssessmentRequestV1` 直接接受三个可反序列化 facts：`IDR/crates/idr-runtime/src/lib.rs:15-22`。
- Runtime 只调用三个纯函数，没有验证 facts 的来源、相互一致性或证据绑定：`.../idr-runtime/src/lib.rs:48-90`。
- Fast Path、Decision 和 Coordination facts 全是普通布尔值/枚举：`.../guards.rs:6-15,91-102,181-190`。

**可复现**

提交同一请求，同时声称：

```text
fast_path = 全部允许条件
multiple_viable_options = true
coordination.action_planned = true
coordination.authorization_required = false
```

Runtime 会同时产生 `AllowFastPath`、`Decision Required`，并可选择无确认的行动模式。当前没有跨 guard 矛盾校验，更没有 proof。

也可以跳过 Runtime，直接调用公开的 `IntentContractV1::issue`、`DecisionContractV1::issue`、`TurnCoordinationPlanV1::issue`。

**修复**

- 把生产构造器改为 `pub(crate)` 或 capability-token protected。
- 引入 `IntentAdmissionProofV1`、`DecisionNecessityProofV1`、`TurnCoordinationAdmissionV1`。
- Proof 必须绑定 input refs、context snapshot、facts digest、policy revision、rule version、issuer、时间和目标 contract。
- Runtime 对 Fast Path、Decision、Turn facts 做交叉一致性校验。

---

## P0-02：Canonical Input 只有角色矩阵，没有可信来源证明

**证据**

`CanonicalInputEventV1` 可通过 JSON 反序列化；校验只验证 UUID、引用、摘要格式、actor-role 组合和 logical time：`.../input.rs:41-118`。没有认证 session、Gateway signature、nonce、channel binding 或 replay record。

**可复现**

构造合法 JSON：

```json
{
  "source_actor": "user",
  "actor_ref": "actor:victim",
  "primary_semantic_role": "authorization",
  "semantic_roles": ["authorization"],
  "content_digest": "sha256:<64 lowercase hex>",
  "logical_time": 1
}
```

只要其他 shape 合法，协议层无法判断它是否真的来自用户。

**修复**

Protocol Gateway 必须将认证上下文签发为 `InputProvenanceProofV1`；Authorization 角色必须要求用户/审批者签名或可信 session attestation；event ID 与 nonce 进入 durable replay index。

---

## P0-03：Exact Action Authorization 仍可伪造，没有 issuer、签名和撤销

**证据**

- Authorization 派生 `Deserialize`：`.../action.rs:193-206`。
- `validate_for` 只检查字段与 Action 一致及时间范围：`.../action.rs:239-264`。
- 没有 issuer、key ID、signature、grant refs、revocation epoch 或 authentication context。
- Action Admission 权限和 policy 仍由裸布尔 facts 提供：`.../action.rs:275-289,308-382`。
- `valid_at` 由调用方传入：`.../action.rs:308-313`。

**可复现**

读取公开 Action 后，离线计算/复制 action ref、parameter digest、actor、scope、Authority Context digest，构造 `decision=approve` 的 JSON Authorization；选择一个处于 issued/expires 区间的调用方 `valid_at`。协议无法区分合法 issuer 与伪造者。

**修复**

Authorization 改为签名 attestation，至少包含 issuer/key ID、subject、actor、tenant、scope constraints、operation、Action ref、parameter digest、authority grant refs、nonce、issued/expires、revocation epoch 和 signature。Runtime 使用可信时钟及 key/revocation registry 验证。

---

## P0-04：Response Admission 可伪造且可在过期后重放

**证据**

- `RenderedResponseEnvelopeV1` 仍可反序列化：`.../response.rs:188-196`。
- `validate_for` 在后续校验时只能检查摘要格式，无法重新拿到内容计算摘要：`.../response.rs:222-240`。
- `ResponsePolicyFactsV1` 可反序列化且所有政策结论仍是布尔值：`.../response.rs:255-269`。
- Admission 没有 `valid_at` 参数，也不检查“发送当前时刻 <= expires_at”；它只检查 `expires_at >= evaluated_at` 且不晚于 Response 有效期：`.../response.rs:287-313`。

**可复现**

1. 反序列化一个 content digest 格式合法的 envelope。
2. 构造与其 ref/digest 相同、全部 policy 布尔值为 true 的 facts。
3. 将 `evaluated_at=100`、`expires_at=101`。
4. 在任意未来时刻调用 Admission，仍可返回 Allow，因为函数不知道当前发送时间。

**修复**

- Admission 消费真实 rendered bytes 或不可伪造的 renderer attestation。
- Policy Evaluation 必须由可信 Policy Engine 签名。
- `admit_for_send(response, bytes, proof, trusted_now)` 必须在唯一发送 API 内执行。
- Admission token 一次性或有明确可复用策略，并持久记录发送。

---

## P0-05：Human Model Gate Decision 虽不可反序列化，但其 facts 可任意伪造且未绑定候选/主体/证据

**证据**

- Gate facts 是可反序列化普通布尔值：`.../human_model.rs:324-334`。
- `explicit_user_confirmation=true` 直接生成 `PromoteUserConfirmed`：`.../human_model.rs:392-420`。
- Assertion 只消费 gate decision 推导 epistemic status，但 decision 不包含 candidate ref、subject、predicate、value digest、user event 或 Outcome ref：`.../human_model.rs:176-235,347-389`。
- Assertion 校验没有要求 `subject_ref == metadata.authority_context.subject_ref`：`.../human_model.rs:269-306`。
- Store 对 Human Model Assertion 没有任何跨对象复核：`idr-store/src/lib.rs:614`。

**可复现**

```text
evaluate_human_model_update_gate({ explicit_user_confirmation: true, ... })
→ PromoteUserConfirmed
```

随后用该 decision 创建 assertion，但把 `subject_ref` 设置为另一用户，source/evidence 用普通引用填充。当前协议和 store 都不会证明用户真的确认，也不会发现跨主体写入。

**修复**

分离 `HumanModelCandidateV1`、`HumanModelPromotionDecisionV1`、`HumanModelAssertionV1`。Promotion 必须绑定候选摘要、subject、predicate/value、HumanConfirmationProof 或 OutcomeObservationProof。Store 强制 same subject/tenant/context，并保存 promotion proof。

---

## P0-06：Store 重放不会重新推导递归失效集合，删除失效标记即可恢复旧合约

**证据**

- 正常更新时通过 `invalidate_dependents` 写入 `invalidated_refs`：`idr-store/src/lib.rs:316-348`。
- 重放校验只检查列表中的引用存在，不计算“按历史本应失效的完整闭包”：`.../idr-store/src/lib.rs:362-440`。
- `load_current` 只查看该引用是否出现在 `invalidated_refs`：`.../idr-store/src/lib.rs:258-285`。
- `claim_action_execution` 也只检查 current pointer 和 invalidated list：`.../idr-store/src/lib.rs:215-221`。

**可复现**

1. 运行现有递归失效测试场景，得到 Intent V2 和被标记失效的下游 Action/Response。
2. 关闭进程，编辑 store JSON，仅从 `invalidated_refs` 删除某个旧 Action ref，不改 records/current pointers。
3. 重新 `open()`；`validate_snapshot` 会通过，因为列表中剩余项都合法。
4. `load_current` 会重新返回旧 Action；配合 `dependencies_current=true` 可进入 claim。

**修复**

不要把 `invalidated_refs` 当作可独立信任的持久事实。重放时从 revision history 和 dependency DAG 重新计算预期失效闭包，并与存储值完全相等；更好的是持久化不可变 invalidation event/hash chain，并把 store root hash 绑定到 admission proof。

---

## P0-07：Store 可被旧快照回滚，空文件会被当成全新合法 store

**证据**

- 持久化只是把完整 JSON snapshot 写临时文件并 rename：`idr-store/src/lib.rs:619-639`。
- 没有 snapshot sequence、previous root hash、签名或外部 anti-rollback anchor。
- 已存在但长度为 0 的文件直接返回 `default()`：`.../idr-store/src/lib.rs:642-651`。

**可复现**

- 保存任一历史合法 snapshot，执行若干新 commit 后用旧文件覆盖当前文件；`open()` 只验证旧 snapshot 内部一致性，不知道它已回滚。
- 将 store 文件截断为 0 字节，`open()` 得到空 store，不报损坏。

**影响**

授权消费、execution claim、撤销、Outcome 和 Human Model 历史都可能被整体回滚或静默清空。

**修复**

采用 append-only log/WAL、单调 sequence、previous hash、checkpoint root、外部 durable anchor；已存在空文件必须返回 corruption。恢复必须区分“文件不存在”和“文件损坏”。

---

## P0-08：Store 允许同一 kind/id/revision 的不同 digest 分叉记录

**证据**

`validate_snapshot` 唯一性使用完整 `HumanCenteredContractRefV1`，其中包含 digest；不同 digest 因而被视为不同记录：`idr-store/src/lib.rs:365-375`。构建 `latest_refs` 时，第二条相同 revision 只会被忽略，不会报 fork：`.../idr-store/src/lib.rs:377-420`。

**可复现**

在 snapshot 中放入两个均可自洽验证、但 `kind + contract_id + revision` 相同且内容/digest 不同的 contract。将 current pointer 指向先出现者。当前算法不会因同 revision 分叉本身报错。

**修复**

建立唯一键 `(kind, contract_id, revision)`，发现第二个不同 digest 必须 `CorruptSnapshot`。同一 contract lineage 还应强制每个 revision 唯一且 predecessor 链连续。

---

## P0-09：Execution claim 将“已预留”混同为“已执行”，崩溃后会永久误判

**证据**

- claim 只有 action、authorization ID、租户、operation、idempotency key 和 `claimed_at`，没有状态、owner、lease、expires、attempt 或 completion：`idr-store/src/lib.rs:89-98`。
- claim 一经持久化，后续同 Action/授权/幂等键直接返回 `AlreadyExecuted`：`.../idr-store/src/lib.rs:224-255`。
- 没有 release、renew、complete、fail/recover API。

**可复现**

1. `claim_action_execution` 返回 Admit 并持久化 claim。
2. 在真正调用外部系统前进程崩溃。
3. 重启后再次 claim，返回 `AlreadyExecuted`，尽管没有任何外部执行和 Receipt。

**修复**

引入 durable execution state machine：`Reserved → Dispatched → Acknowledged/Unknown → Verified/Failed/Compensated`。Claim 必须有 claim ID、owner、lease、attempt、deadline、recovery policy。`AlreadyExecuted` 只能在存在已验证完成 Receipt/Provider idempotency proof 时返回；未决状态应返回 `ExecutionStateUnknown/Recover`。

---

## P0-10：Execution Receipt 没有绑定不可伪造 permit，Provider 和执行事实均无签名

**证据**

- Receipt 没有 authorization ref、action admission ref、claim/permit ref：`.../execution.rs:21-40`。
- `issue` 是公开函数，输入 provider、时间、结果、verification 都由调用方提供：`.../execution.rs:43-90`。
- Store 只要求存在任意相同 Action 且 `claimed_at <= started_at` 的 claim：`idr-store/src/lib.rs:554-568`。
- Claim 没有不可伪造 token，Receipt 也不引用具体 claim ID。

**可复现**

对已有 claim 的 Action，调用公开 `ExecutionReceiptV1::issue`，自行填写 provider、Succeeded、result payload 和 verification 文本。Store 只能确认“以前有 claim”，无法确认该 Receipt 来自真实执行器或对应具体 dispatch。

**修复**

Store 原子签发签名/opaque `ExecutionPermitV1`；执行器只接受 permit；Receipt 必须引用 permit ID、authorization/admission refs、attempt、provider identity，并由 provider/runtime 签名。Store 一次性完成 permit state transition。

---

## P0-11：同一 contract revision 可切换 run/turn/tenant/subject/authority context

**证据**

`HumanCenteredContractMetadataV1::successor` 接受新的 run ID、turn ID 和 Authority Context；只检查 predecessor kind/id/revision：`.../common.rs:220-253,287-317`。Store commit 也只检查 pointer 和 predecessor ref：`idr-store/src/lib.rs:146-162`。

**可复现**

为 Tenant A 的 Action V1 创建 successor metadata，但传入 Tenant B/Subject B 的 Authority Context；构造 Action V2 并 commit。当前 lineage 校验不会拒绝。

**影响**

一个逻辑 contract ID 可以跨租户/主体迁移，破坏审计、权限边界和递归失效语义。

**修复**

定义每种 contract 的 lineage invariants。通常 contract ID 的 tenant、subject、authority root、run ID 不得变；允许跨 turn 时必须显式定义。Store 在 successor commit/replay 时比较 predecessor metadata，并要求合法 migration proof。

---

# 7. P1 问题

## P1-01：Medium/Low 的不可逆 Action 可以完全跳过 Decision

`ActionContractV1` 只对 High/Critical 强制 `decision_ref`：`.../action.rs:128-132`。但 Decision Necessity 设计把不可逆结果视为必须进入 Decision Runtime：`.../guards.rs:121-177`。

**复现**：构造 `impact=Medium`、`reversible=false`、`decision_ref=None`、`authorization_state=NotRequired` 的 Action；协议和 store 的 Action branch 都不会要求 Decision。

**修复**：Action 是否必须引用 Decision 应由 `DecisionNecessityAdmission` 决定，而不是只依据 impact enum。

---

## P1-02：Outcome 有结构链接，但没有 Observation Attestation，也不复核 Receipt 状态与归因

Outcome 的 observed results、success criteria、evidence 和 attribution 都由调用方填写：`.../outcome.rs:29-87`。Store 只验证 Action/Receipt 引用一致和上下文一致：`idr-store/src/lib.rs:570-613`，不检查 Receipt 是否 Succeeded、Failed、Rejected 或是否支持该归因。

**复现**：为 Failed Receipt 创建 `state=Observed`、`attribution=ExecutionQuality`、success criteria 全部 satisfied 的 Outcome；只要引用一致和有普通 evidence ref，可能通过。

**修复**：新增 `OutcomeObservationAttestationV1`，绑定 observer、time window、source payload digest、Receipt state、success criteria evaluation 和签名。

---

## P1-03：一个 claim 可以支持多个不同 Receipt

Store 只检查“存在某个 claim”，没有消费/完成 claim，也没有 Receipt 唯一性键：`idr-store/src/lib.rs:554-568`。因此可以提交多个不同 execution ID/结果的 Receipt，均指向同一 Action/claim。

**修复**：Receipt 必须绑定 claim/permit ID；permit state transition 使用 CAS；按 attempt/provider external ID 定义唯一性和补充/替换规则。

---

## P1-04：Orchestration Runtime 与原始设计差距仍是架构级缺失

`idr-runtime` 只有一次 `assess()`：`idr-runtime/src/lib.rs:44-90`。没有 durable run/turn/step、wait gate、timeout、retry、cancel、interruption/recovery、parallel join、recomposition 或 invalidation handling。

**修复**：建立事件驱动 orchestration state machine，所有 transition 写入 durable log，并要求 transition admission proof。

---

## P1-05：Store 不是原始设计要求的 immutable Audit Ledger

原始设计要求 immutable event、hash chain、transactional outbox：`source-design/IDR-V1.3-original-design.txt:1043-1087`。当前 store 每次重写完整 snapshot：`idr-store/src/lib.rs:619-639`。

**修复**：snapshot 只能作为派生 checkpoint；权威事实必须来自 append-only journal，使用 chained digest/sequence 和 outbox。

---

## P1-06：Cross-language wire semantics 仍有漂移

1. Rust `u64` 可大于 `2^53-1`；TS `requiredPositiveInteger` 拒绝非 safe integer：`packages/interaction-client/src/index.ts:349-353`。
2. Rust UUID newtype 主要拒绝 nil；TS regex 额外限制 version 1–8 和 RFC variant：`.../index.ts:422-434`。
3. Rust 多数对象使用 `deny_unknown_fields`；TS `asRecord` 和 Python mapping 会忽略未知字段：`.../index.ts:448-455`；`contracts.py:46-79`。
4. Python CanonicalInputObservation 只保留 source_actor、roles、content ref/digest，不验证 event/run/turn UUID、actor_ref、correlation_ref、logical_time、schema version：`contracts.py:38-79`。
5. 三种语言手写枚举与 actor-role 矩阵，没有 schema code generation。

**修复**：冻结 language-neutral schema；u64 采用十进制字符串或统一 ≤2^53-1；生成 validators；对 unknown fields 统一策略。

---

## P1-07：摘要编码尚未跨语言规范化，也没有真实性

Rust digest 基于 `serde_json::to_vec(&(domain, value))`：`.../common.rs:365-374`。没有 RFC 8785/JCS 或等价 canonical encoding，也没有 signature。Map key 顺序、数字、Unicode 等在跨语言重算时可能漂移。

**修复**：定义 IDR Canonical Encoding V1，提供 canonical byte vectors、digest vectors 和 signature vectors，在 Rust/TS/Python CI 中逐字节比较。

---

## P1-08：Aegis adapter 是单向的，但没有权限衰减且直接重导出 raw runtime/store

adapter 将 Aegis Authority Context 字符串逐项复制：`Aegis-Life/crates/idr-aegis-adapter/src/lib.rs:18-35`，没有验证 scope 是否收窄、tenant/subject 是否受宿主认证证明支持。同时 `pub use idr_runtime` 和 `pub use idr_store`：`.../lib.rs:14-16`，宿主可直接访问裸 facts admission 和 store claim API。

**修复**：adapter 只暴露高层 façade；Authority mapping 必须消费 Aegis-signed host proof 并执行 monotonic attenuation；不要重导出 raw trust-root crates。

---

## P1-09：Store 文件锁只覆盖单机共享文件，不提供多机一致性

`fs2::lock_exclusive` 作用于本地/共享文件语义：`idr-store/src/lib.rs:654-668`。没有 fencing token、distributed consensus 或数据库事务隔离。

**修复**：生产环境使用具备线性化 CAS/事务能力的 durable store，或明确限定为单节点 shadow-mode backend。

---

## P1-10：测试仍缺少决定性攻击、故障和并发矩阵

当前 Rust 测试数量：protocol 13、runtime 2、store 7；TS 6、Python 5。缺失至少包括：

- 删除 `invalidated_refs` 后 reopen；
- 空文件、旧 snapshot 回滚、同 revision fork；
- claim 后崩溃、lease 超时、未知执行恢复；
- 多 Receipt/同 claim；
- forged Authorization/Policy/Human Confirmation；
- caller backdated `valid_at`；
- cross-tenant successor；
- Outcome 与 Receipt state/verification 冲突；
- fault injection：write、fsync、rename、directory sync 各阶段失败；
- 多进程而非仅同进程双实例；
- canonical bytes/digest/signature vectors；
- Aegis privilege widening。

---

# 8. P2 问题

## P2-01：Decision Stage 仍没有记录用户最终选择

`UserDecided` 只要求 recommendation 存在且 required inputs 为空：`.../decision.rs:263-277`，没有 `selected_option_ref`、decision maker、decision event、time 或 rationale confirmation。

## P2-02：Decision criteria weight 没有完整规范

每个 weight 只是可选 basis points，没有要求 Required criteria 的处理方式、总和、归一化或缺失含义：`.../decision.rs:31-55,217-286`。

## P2-03：Payload 仍缺少统一总字节/深度预算

集合数量已有部分上限，但 Action parameters、Execution result payload 和嵌套 JSON 没有统一 byte/depth/node budget：`.../action.rs:23-40`；`.../execution.rs:21-40`。

## P2-04：Response Policy 有效期允许 `expires_at == evaluated_at`

当前只拒绝 `expires_at < evaluated_at`：`.../response.rs:307-313`。若定义为闭区间需要规范；若期望实际可用窗口，应要求严格大于并在发送时检查。

## P2-05：文档中“append-only store”的表述仍容易误导

实现会保留 records，但物理介质是全量 snapshot rewrite，不是 append-only audit substrate。应在 README/架构文档中明确区分 logical append history 与 physical immutable ledger。

---

## 9. Rust、TypeScript、Python 职责边界结论

### Rust

没有发现 Rust 把 UI 或离线模型评估纳入核心。但 Rust 当前**没有封闭唯一生产入口**：大量 authoritative contract 的 `issue` 为公开函数，facts 和证明对象可从外部输入。问题不是 Rust 做了 TS/Python 的工作，而是 Rust 尚未真正垄断裁决、签发和执行能力。

### TypeScript

当前代码只做 transport shape validation、解析 assessment 和构造 exact Action 授权提交，没有复制 guard 规则，职责边界基本正确。问题是它的验证器与 Rust wire semantics 尚未由单一 schema 生成。

### Python

当前只加载 fixture、比较 Runtime 输出和检测基础意图删除/确认绕过，没有生产写入能力，职责边界正确。但其 Canonical Input 模型只是评估用投影，不等价于完整协议验证器；文档必须避免把它表述成跨语言完整 conformance。

---

## 10. Action 授权专项结论

### 已做到

- 精确 Action kind/ID/revision/record digest；
- parameter digest；
- actor；
- scope；
- Authority Context digest；
- issued/expires；
- store 层 authorization ID 防重复 claim。

### 仍缺失

- 可信 issuer；
- signature/MAC；
- key ID/rotation；
- Authority Grant refs；
- authentication method/session binding；
- revocation registry/epoch；
- trusted current time；
- Action authorization artifact 的持久保存和审计；
- claim/permit 的不可伪造性；
- crash recovery 与 unknown-execution handling。

因此，授权“精确绑定”已部分实现，但“授权真实性和只消费一次”尚未端到端实现。

---

## 11. 合约依赖、版本替换和递归失效专项结论

### 正常 API 路径

- exact dependency 必须存在且为 current；
- predecessor 连续；
- 上游换版会递归标记下游 current refs；
- cross-object context/linkage 有复核。

### 不可靠处

- 重放不重算失效闭包；
- snapshot 可回滚；
- 空文件被视为新 store；
- same revision fork 不拒绝；
- successor 可改变 tenant/subject/run/turn/context；
- 没有不可变日志和外部 anti-rollback anchor。

所以“正常 commit 中的递归失效算法”基本正确，但“持久状态经过篡改、回滚或崩溃后的可靠性”不充分。

---

## 12. Execution Receipt、Outcome Record、Human Model 分离专项结论

类型层面已经分离，没有把 Receipt 字段直接塞入 Outcome 或 Human Model，这部分正确。

但信任语义尚未分离完整：

- Receipt 没有 permit/authorization/admission proof；
- Outcome 没有 observation proof；
- Human Model promotion 没有绑定 Outcome/Human Confirmation；
- Store 对 Human Model 没有跨对象复核；
- Outcome 可在结构引用正确时表达与 Receipt 实际状态不一致的结论。

因此是“数据结构分离”，还不是“证据链和裁决职责分离”。

---

## 13. 下一阶段实施顺序

### 阶段 0：保持发布阻断

```text
PRODUCTION_TRUST_ROOT = BLOCKED
PRODUCTION_ACTION_EXECUTION = FORBIDDEN
LONG_TERM_HUMAN_MODEL_WRITE = SHADOW_ONLY
AEGIS_PRODUCTION_AUTHORITY_BRIDGE = FORBIDDEN
```

### 阶段 1：先修 Store 完整性与恢复语义

1. 空文件必须 Corrupt；
2. 重放重算 invalidation closure；
3. 拒绝 same `(kind,id,revision)` fork；
4. successor 上下文连续性；
5. append-only journal、sequence/hash chain、anti-rollback；
6. fault injection 和 crash matrix。

原因：在持久层仍可回滚/删失效标记时，任何签名 proof 都可能被旧状态重放。

### 阶段 2：建立 Proof Framework 与可信时间

实现 issuer registry、signature verification、purpose/tenant/scope constraints、revocation、key rotation、trusted clock。优先定义：

```text
InputProvenanceProofV1
AuthorityGrantProofV1
PolicyEvaluationProofV1
CapabilityProofV1
DependencySnapshotProofV1
HumanConfirmationProofV1
OutcomeObservationProofV1
```

### 阶段 3：封闭 Runtime 和 Contract 构造入口

- 所有 authoritative `issue` 改为 crate-private；
- 外部只能提交 Candidate/Input/Proof；
- Guard Decision 必须绑定 facts digest 和输入证据；
- 后续 Contract 强制依赖 admission record。

### 阶段 4：重构 Execution Permit 状态机

```text
Reserved
→ Dispatched
→ Acknowledged | Unknown
→ VerifiedSucceeded | VerifiedFailed
→ Compensated
```

Permit 具备 claim ID、lease、owner、attempt、deadline、provider binding、signature。Receipt 必须绑定 permit。

### 阶段 5：闭合 Outcome 与 Human Model

Outcome 只能由 Observation Proof 生成；Human Model 采用 Candidate→Promotion→Assertion 三阶段，subject/tenant/evidence 全链绑定。

### 阶段 6：完成 Orchestration Runtime

实现 durable run/turn/step、等待、授权 gate、timeout、retry、cancel、interruption recovery、parallel join、invalidation/recompute。

### 阶段 7：冻结跨语言协议

单一 schema 源生成 Rust/TS/Python；冻结 canonical encoding 和 test vectors；统一 UUID、u64、unknown fields、文本/集合预算。

### 阶段 8：最后接入 Aegis Life

adapter 只暴露 high-level façade；Aegis proof 必须经过 IDR 验证和权限衰减；禁止重导出 raw store/runtime 作为产品 API。

---

## 14. 发布验收门槛

只有同时满足以下条件，才能从 `BLOCKED` 进入下一状态：

```text
P0 = 0
Store tamper/rollback/crash matrix = PASS
Invalidation recomputation = PASS
Execution unknown/recovery matrix = PASS
Signed proof + revocation suite = PASS
Authorization replay/permit replay = PASS
Outcome/Human Model provenance suite = PASS
Cross-language canonical vectors = PASS
Aegis privilege attenuation = PASS
Independent cargo test/clippy/fmt = PASS
Independent TS test/typecheck = PASS
Independent Python test = PASS
```

## 最终判断

IDR V1.3 当前已经具备较好的协议基础、局部不变量和 shadow-mode 验证价值。整改不是表面性的：contract kind、validated deserialization、Action expiry、Turn DAG、Response 内容摘要、持久 claim、文件锁和跨对象一致性都是真实进展。

但它仍不是生产信任根。当前最危险的误判是把“字段绑定、摘要和本机 JSON 持久化”视为“可信证明、不可回滚账本和真实执行”。下一阶段不应继续扩充普通 Contract 字段，而应优先完成 **Store anti-rollback + proof framework + execution permit/recovery** 三个基础层。
