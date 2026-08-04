The Human-Centered Intent & Decision Runtime V1.3

Round 3 严格复审报告

审计日期：2026-07-28审计对象：IDR-V1.3-reaudit-round3-2026-07-28.zip正式名称：The Human-Centered Intent & Decision Runtime（IDR）审计方式：安全解压、清单校验、Round 2→Round 3 差异审查、完整静态源码审查、可运行测试复跑、攻击路径推演

一、总体结论

PRODUCTION TRUST ROOT = BLOCKED
FOUNDATION / SHADOW MODE = ACCEPTABLE WITH LIMITATIONS
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED

Round 3 是一次实质性整改，不是简单补文档。新增了 Ed25519 Proof Framework、ExecutionPermitV1、Orchestrator 协议表面，修复了 Round 2 的多项结构问题，并把完整 Aegis Life Rust 工作区纳入审计。

但是，Round 3 仍不能成为生产信任根。根本原因没有改变：

Rust 中已经出现了“证明、许可、编排”的类型，但它们尚未组成唯一、不可绕过、可持久验证的生产路径；部分新增安全类型自身还存在可直接利用的绑定错误。

本轮共确认：

已关闭/基本关闭的重要问题：9 类

P0：14 项

P1：10 项

P2：6 项

其中最严重的新发现是：

ExecutionPermitV1 可以使用 Deny 授权成功构造；

Permit 中的 provider、owner、attempt、lease、dispatch nonce 和 authorization ID 没有被签名 Proof 覆盖；

Permit 租期可以超过用户授权有效期；

Store 的递归失效逻辑只比较 contract_id，不同 Contract Kind 复用同一个 UUID 时可以跳过失效传播；

Execution claim 没有 lease/owner/state/recovery，失败回执又会阻止后续重试，形成永久悬挂。

包自身把生产功能固定为 blocked，并明确承认 Proof、Permit、Orchestrator 尚未端到端接入，这是正确且诚实的发布姿态：README.md:6-18、IDR/docs/audit/IDR_V1_3_REMEDIATION.md:9-13,46-68。

二、审计包与独立验证

2.1 安全解压与完整性

ZIP 条目：457

有效文件：334

SHA256SUMS 条目：333（清单自身不计入）

SHA-256：333/333 通过

路径穿越：0

符号链接：0

主要目录：IDR/、Aegis-Life/、source-design/

2.2 独立测试

项目

结果

说明

TypeScript Node tests

8/8 PASS

独立复跑

Python unittest

7/7 PASS

独立复跑

TypeScript typecheck

未完成

包未包含 node_modules，当前离线环境缺少 @types/node；失败仅为模块声明缺失，不据此判源码错误

IDR Rust tests

未独立运行

当前审计环境没有 cargo/rustc

Aegis Rust tests

未独立运行

同上

维护者声明 Rust 34 项、Aegis Adapter 2 项、Clippy、fmt、workspace check 均通过，但这些只能视为维护者证据，不能替代独立执行：VALIDATION-SUMMARY.md:5-20。

三、Round 2 → Round 3 整改结果

3.1 已确认关闭或明显加强

Assessment 与 Turn Plan 不可表示问题已关闭Runtime 与 selector 现在都拒绝 response_planned=false：IDR/crates/idr-runtime/src/lib.rs:103-121、IDR/crates/idr-protocol/src/human_centered/guards.rs:204-228。

Low-impact 不可逆 Action 跳过 Decision 已关闭任意不可逆 Action 都必须引用 Decision：IDR/crates/idr-protocol/src/human_centered/action.rs:130-135。

Store revision fork 检测已实现(kind, contract_id, revision) 必须唯一：IDR/crates/idr-store/src/lib.rs:428-443。

失效集合重放闭包校验已实现Snapshot 中的 current pointers 与 invalidated refs 必须等于历史重放结果：IDR/crates/idr-store/src/lib.rs:504-515。

空 Store 和损坏摘要处理已加强sequence、previous digest、snapshot digest 之间有结构校验：IDR/crates/idr-store/src/lib.rs:405-427。

lineage context 连续性已加入 Storesuccessor 要求 run、turn、authority context 一致：IDR/crates/idr-store/src/lib.rs:179-187,626-633。

Response 内容摘要由 Rust 根据真实内容计算并复核RenderedResponseEnvelopeV1::new 消费实际文本，Admission 再次检查：IDR/crates/idr-protocol/src/human_centered/response.rs:198-257,304-340。

Turn DAG、逐 Action 授权 Gate 和顺序约束已加强IDR/crates/idr-protocol/src/human_centered/turn.rs:113-218,245-305。

Aegis Authority 直拷贝问题已关闭于 shadow 边界Adapter 只返回 HostAuthorityCandidateV1，不产生 IDR Authority Context：Aegis-Life/crates/idr-aegis-adapter/src/lib.rs:23-79。

3.2 新增但尚未闭环的基础

Ed25519 signed Proof：trust.rs

Opaque VerifiedProofV1

Opaque ExecutionPermitV1

Orchestration Run/Step/Command/Repository 接口

Production feature gates 固定为 blocked

这些新增方向正确，但不能把“类型存在”当作“生产链路已经强制执行”。包自身也明确承认 Admissions、Store 和 Executors 尚未端到端消费它们：README.md:16-18。

四、十项审核结论

审核项

Round 3 结论

1. 原始设计与实现逐项对应

部分对应。Data Plane 合约和确定性规则最完整；Control Plane、Gateway、Context Fabric、真实 Audit Ledger、Cognitive Services 和 Outcome Learning 尚未形成生产实现

2. Rust/TypeScript/Python 边界

总体遵守。TS 只做 transport/interaction validation，Python 只做离线评估；但 Rust 内部权威对象仍可由普通调用者构造或反序列化

3. 五类门禁绕过

仍可绕过。Intent/Decision/Action/Response/Human Model 仍消费调用方 facts 或可反序列化对象，Proof 尚未成为强制前置依赖

4. Action 授权精确绑定

字段绑定改善，真实性未解决。ID/revision/digest/parameters/actor/scope/context 已绑定；issuer signature、trusted clock、revocation、single-use 未闭合；新增 Permit 还有 Deny 授权漏洞

5. 合约依赖/替换/递归失效

明显改善但仍不可靠。重放、fork、currentness 已加强；存在跨 Kind 同 UUID 失效绕过，且 anti-rollback 仍仅本地双文件

6. Receipt/Outcome/Human Model 分离

类型分离正确，证明链缺失。Receipt 不绑定 Permit，Outcome 不绑定 Observation Proof，Human Model 不绑定 Candidate/Promotion Proof

7. 状态存储持久性/原子性/重放/损坏检测

foundation 级可接受，生产级不足。有锁、rename、fsync、摘要链、anchor；无 WAL/outbox、多机一致性、外部不可变锚、完整 claim 生命周期

8. 跨语言协议/fixture

部分一致。actor-role、unknown fields、safe integer 已对齐；仍无单一 schema 生成、JCS、Proof signing golden vectors，UUID 规范化行为仍不同

9. Aegis Life 反向耦合/权限泄漏

当前 shadow adapter 未发现反向耦合或直接授权泄漏。但它仍可提交带裸 facts 的 Assessment Request，生产权限衰减尚未实现

10. 测试攻击/失败/并发覆盖

不足。已有正常路径、部分损坏/并发测试；缺少 Permit Deny、Proof replay、跨 Kind 失效、claim crash/retry、HM promotion、provider forgery 等关键攻击测试

五、P0 问题

P0-01：权威 Contract 创建与持久化路径仍未封闭

代码证据

核心权威对象仍同时具备 Deserialize 和公开 issue()：

Intent：intent.rs:146-164

Decision：decision.rs:127-151

Turn Plan：turn.rs:51-68

Response：response.rs:48-68

Action：action.rs:23-44

Receipt：execution.rs:131-155

Outcome：outcome.rs:29-50

Human Model Assertion：human_model.rs:147-178

Store 又公开接受完整枚举对象并直接 commit：IDR/crates/idr-store/src/lib.rs:28-39,140-218。

可复现方式

任意 Rust 调用者可以：

调用公开 issue() 生成权威 Contract；或

从 JSON 反序列化一个形状和 digest 合法的 Contract；

包装为 HumanCenteredContractSnapshotV1；

调用 store.commit()。

它不需要 Runtime-issued proof、sealed envelope 或 issuer attestation。

影响

Rust 是语言边界，但不是唯一权威颁发路径。只要代码运行在同一进程或能写入 Store 输入，就能制造“权威”合约。

推荐修复

区分 WireCandidateV1、ValidatedCandidateV1、RuntimeIssuedContractV1、StoredEnvelopeV1；

权威 Contract 不实现公开 Deserialize；

issue() 改为 pub(crate) 或仅暴露给 issuer service；

Store 只接受由 Runtime signing key 封装的 SealedContractEnvelopeV1；

replay 同时验证 envelope issuer、signature、sequence 和 policy revision。

P0-02：Intent Fast Path、Decision Necessity、Turn Coordination 与 Action Admission 仍消费裸 facts

代码证据

Runtime request 直接包含三组可反序列化 facts：IDR/crates/idr-runtime/src/lib.rs:19-26。Action Admission 直接消费十个布尔字段：action.rs:278-291,311-377。Store claim 继续接收这些 facts：IDR/crates/idr-store/src/lib.rs:221-230。

新增 VerifiedProofV1 没有被上述 API 强制要求。

可复现方式

调用者把 capability、actor authority、agent authority、parameters、preconditions、policy、verification、compensation、dependencies 全部设置为 true，即可令纯函数返回 Admit；只要 Store 中 Action current，claim 就会被持久化。

推荐修复

把 facts 拆成不可伪造证明：

VerifiedCapabilityProof
VerifiedActorAuthorityProof
VerifiedAgentAuthorityProof
VerifiedParameterValidationProof
VerifiedPreconditionSnapshot
VerifiedPolicyDecision
VerifiedDependencySnapshot

Admission 必须在锁内从权威 Store/Registry 读取当前版本并验证 proof，不接受外部布尔结论。

P0-03：Canonical Input 只有形状限制，没有来源认证与重放防护

代码证据

CanonicalInputEventV1 可以反序列化，并允许调用者直接填写 source actor、actor ref、run、turn、content digest 和 logical time：input.rs:41-90。validate() 只检查 UUID、摘要格式、actor-role 允许矩阵和 safe integer：input.rs:92-118,150-167。

可复现方式

攻击者构造 source_actor="user"、合法 actor_ref 和允许角色，并提供任意 sha256:<64 hex> 即可得到形状合法的“用户输入”。没有 Gateway signature、session binding、channel binding 或 nonce registry 证明它真的来自该用户。

推荐修复

实现：

AuthenticatedIngressEnvelope
GatewayInputAdmissionProof
session_id/channel_id/device_id
source credential / authentication context
nonce + replay index
received_at from trusted clock
raw payload digest + canonical event digest

Canonical Input 只能由 Protocol Gateway 消费认证 envelope 后颁发。

P0-04：Exact Action Authorization 仍不是真实授权证明

代码证据

Authorization 仍可反序列化，且 issue() 公开：action.rs:196-240。validate_for() 只核对 Action、参数、actor、scope、Authority Context 摘要和时间，没有 issuer、signature、authentication context、revocation registry 或 one-time consumption：action.rs:242-267。

可复现方式

同进程普通调用者对任意 Action 调用 ExactActionAuthorizationV1::issue(... Approve ...)；只要 actor/scope 与 Action Context 相同，就获得结构合法授权。

推荐修复

Exact Authorization 必须只能从 VerifiedProofV1<AuthorizationApproval> 产生，并绑定：

issuer/key ID；

authenticated user event；

Action ref + parameter digest；

actor/subject/tenant/scope/purpose/operation；

policy revision；

issued/expires；

revocation epoch；

one-time nonce/consumption state。

P0-05：ExecutionPermitV1 接受 Deny 授权，并允许 Permit 超出授权有效期

代码证据

ExecutionPermitV1::from_verified_proof 只调用：

authorization.validate_for(action, verified_at)?;

但没有检查 authorization.decision == Approve：execution.rs:43-57。ExactActionAuthorizationV1::validate_for 本身也不检查 decision：action.rs:242-267。

Permit 的 lease_until 只受 Proof expiry 和 Action valid_until 限制，没有受 Authorization expires_at 限制：execution.rs:68-70。

可复现方式

创建 AuthorizationDecisionV1::Deny 的 Authorization；

创建并验证 ProofKindV1::ExecutionPermit Proof；

调用 ExecutionPermitV1::from_verified_proof；

当前代码可以返回成功。

另一个路径：Authorization 在 T+10 过期，Proof 在 T+100 过期，传入 lease_until=T+50，当前检查允许 Permit 在用户授权失效后继续有效。

推荐修复

强制 authorization.decision() == Approve；

lease_until <= authorization.expires_at()；

授权撤销必须在 dispatch 和 provider execution 前再次检查；

增加 validate_at_execution(trusted_now, revocation_snapshot)；

添加 Deny、expired、revoked、lease-overrun 攻击测试。

P0-06：Execution Permit 的关键字段没有被签名 Proof 覆盖，可被同一 Proof 任意改写和重复派生

代码证据

Proof verifier只把 expected kind、subject digest、tenant 作为外部期望：trust.rs:322-384。Permit 只要求 Proof subject_digest == action.record_digest，然后由调用者另行传入 provider、owner、attempt、lease_until、dispatch_nonce 和 Authorization：execution.rs:43-88。

VerifiedProofV1 还实现了 Clone：trust.rs:387-408。

可复现方式

对同一个 cloned VerifiedProofV1，分别调用两次：

provider=A, owner=X, attempt=1, nonce=N1
provider=B, owner=Y, attempt=99, nonce=N2

两份 Permit 都可成功。签名者实际只签了 Action digest，并没有签发这些执行约束。

推荐修复

Proof 的 subject digest 应覆盖完整 ExecutionPermitRequestV1：

action_ref
authorization_id + authorization_digest
provider_ref
owner_ref
attempt
lease_issued_at / lease_until
dispatch_nonce
idempotency scope
policy revision

同时：

VerifiedProofV1 不应任意 Clone；

nonce 必须在持久化 replay index 中原子消费；

Permit 必须单次使用或显式声明可重用次数；

verifier API 要接受 expected subject_ref/scope/purpose/policy revision。

P0-07：Execution claim 没有生命周期，崩溃后可能永久悬挂

代码证据

Claim 记录只有 authorization ID、Action ref、tenant、operation、idempotency key 和 claimed_at，没有 owner、lease、attempt、state、renewal、dispatch status：IDR/crates/idr-store/src/lib.rs:106-115。

只要同一 Action/Authorization/幂等范围已经 claim，后续永远返回 ExecutionPending：idr-store/src/lib.rs:271-283。

可复现方式

claim_action_execution() 成功写入 claim；

进程在真正调用外部 Provider 前崩溃；

重启后重新 claim；

永久得到 ExecutionPending，没有 takeover/reconcile/abandon API。

推荐修复

建立持久状态机：

RESERVED → DISPATCHED → ACKNOWLEDGED → VERIFIED_COMPLETED
                  ↘ UNKNOWN / RECONCILING
RESERVED/DISPATCHED → LEASE_EXPIRED → TAKEOVER | ABANDONED

Claim 必须绑定 Permit、owner、attempt、lease、dispatch nonce、provider 和 expected aggregate sequence，并支持 renew、reconcile、takeover、cancel。

P0-08：Receipt 不绑定 Execution Permit、Authorization 或 Provider 签名，且当前模型阻断合法重试

代码证据

Receipt 字段没有 permit ID、authorization ID、owner、dispatch nonce 或 provider signature：execution.rs:131-150。其公开 issue() 只需要 Action 和普通参数：execution.rs:153-203。

Store 只验证存在一个 Action claim 且 claim 时间不晚于 receipt start：idr-store/src/lib.rs:715-733。它不核对 provider、attempt、authorization 或 Permit。

Store 同时禁止同一 Action 出现第二个 Receipt：idr-store/src/lib.rs:723-729。

可复现方式

任何能构造 Receipt 的代码，在已有 claim 后可填写任意 provider 并提交；

claim 后提交 Failed Receipt；

后续重试仍被旧 claim 判为 pending；即便绕过，第二个 Receipt 也会被“一 Action 一 Receipt”规则拒绝。

推荐修复

Receipt 必须绑定并验证：

permit_id
authorization_ref
action_ref
provider_ref + provider key ID
owner_ref
attempt
dispatch_nonce
request digest
provider response digest
provider signature

同一 Action 应允许多个 attempt receipt，以 (action_ref, attempt) 唯一；另设一个 canonical terminal outcome pointer。

P0-09：Response Admission 仍可由调用方伪造并重放

代码证据

真实内容摘要复核已经正确，但 Policy Facts 仍是可反序列化普通对象和布尔值：response.rs:272-286。evaluate_rendered_response_admission 接受调用方提供的 valid_at：response.rs:304-340。Admission Decision 本身也可反序列化：response.rs:295-302。

可复现方式

调用方填写绑定一致的 response ref/content digest/policy revision，把所有布尔检查设为 true，并选择位于窗口内的 valid_at，即可得到 Allow；没有可信 Policy issuer、trusted clock、nonce 或唯一发送通道。

推荐修复

Policy Engine 签发 SignedResponsePolicyProof；

Runtime 使用可信时间验证；

生成单次 ResponseSendPermit；

只有唯一 Host Output Facade 可以原子消费 Permit；

发送后写 ResponseDeliveryReceipt，禁止重复发送或内容替换。

P0-10：Human Model Gate 仍可被普通调用者伪造，Assertion 与晋级证明没有持久绑定

代码证据

Human Model Assertion 仍可反序列化：human_model.rs:147-176。公开 issue() 接受一个 Gate Decision，但 Gate Decision 来自公开函数和裸 facts：human_model.rs:178-235,330-340,398-453。

调用者只要设置 explicit_user_confirmation=true 就会得到 PromoteUserConfirmed：human_model.rs:421-426。该 Decision 不绑定 candidate、predicate、value、subject、用户输入事件或 Outcome。

Store 对 Human Model Assertion 的跨对象一致性检查为空：idr-store/src/lib.rs:790。

可复现方式

构造 HumanModelUpdateFactsV1 { explicit_user_confirmation: true, ... }；

调用 evaluate_human_model_update_gate()；

用同一个 Decision 为任意 predicate/value 创建多个 LongTerm Assertion；

store.commit() 不验证确认事件或候选对象。

推荐修复

实现不可跳步链：

HumanModelUpdateCandidate
→ SignedPromotionEvidence
  - UserConfirmationInputProof 或 OutcomeObservationProof
→ HumanModelPromotionDecision
→ RuntimeIssuedHumanModelAssertion

每一步绑定 candidate digest、subject、predicate、value、scope、tenant、policy revision 和证据 refs；Assertion 不可 wire Deserialize；Store 必须重放并复核完整链。

P0-11：Outcome 仍可伪造，无法作为 Human Model 学习依据

代码证据

Outcome 仍可反序列化并公开 issue()：outcome.rs:29-86。校验只要求状态、结果文本、evidence refs 和依赖关系自洽：outcome.rs:107-179。没有 observer、observation time、source system signature、attribution review 或 OutcomeObservation Proof。

Store 能检查 Receipt/Action/Decision 的引用一致性，但不能证明 observed result 真的发生：idr-store/src/lib.rs:738-789。

可复现方式

对任意合法 Action/Receipt，填写非空 observed_results 和任意 evidence:* reference，选择 Observed，即可形成结构合法 Outcome。

推荐修复

新增：

OutcomeObservationAttestationV1；

observer/provider identity；

observation window；

evidence object digest；

signed source proof；

Attribution Review；

independent observation threshold。

Outcome Record 只能由 Outcome Learning Runtime 消费 verified observation 后颁发。

P0-12：Orchestration Runtime 仍只是接口和状态类型，不是贯穿链路的控制平面

代码证据

模块头部明确说明没有 production repository 或 scheduler：IDR/crates/idr-runtime/src/orchestrator.rs:1-5。

当前只有：

Step kind/state；

单 Step transition；

wait condition；

retry policy；

Run record 数据结构；

command enum；

repository trait。

见 orchestrator.rs:15-265。

缺失：持久 repository、event loop、scheduler、timeout timer、dependency resolver、Turn Plan executor、contract revision invalidation/recompute、authorization wake-up、provider reconciliation、transactional outbox。

与原始设计差距

原始设计要求 Orchestrator 管理调用顺序、Contract 版本/依赖、等待、并行、超时、重试、取消、中断、恢复、失效传播和 Turn Plan：source-design/IDR-V1.3-original-design.txt:58-69,1010-1041。

推荐修复

先实现单节点 durable orchestrator：

append-only orchestration events
aggregate CAS
scheduler/timer wheel
command inbox + transactional outbox
step dependency DAG
wait registration/wakeup
lease and retry controller
invalidation/recomposition controller
provider reconciliation

随后再做多实例 ownership/partitioning。

P0-13：Store 仍不能抵抗 snapshot 与本地 anchor 同时回滚

代码证据

Store 自己声明它不是 externally anchored immutable production audit ledger：idr-store/src/lib.rs:1-7。

当前 snapshot digest chain 和同目录 append-only anchor 可以检测局部破坏，但如果攻击者把 snapshot 与 .anchor 一起回滚到同一个历史一致点，本地验证仍会通过。

原始设计要求 append-only audit ledger、哈希链和 Transactional Outbox：source-design/...:1043-1087。

推荐修复

PostgreSQL append-only audit events + aggregate sequence；

transactional outbox；

外部 WORM/object lock 或 transparency log；

定期把 ledger root 发布到独立信任域；

recovery 时验证外部 checkpoint，不只依赖同机文件。

P0-14：递归失效存在跨 Contract Kind 同 UUID 绕过

代码证据

invalidate_dependents 跳过条件是：

if pointer.contract_id() == stale.contract_id() { continue; }

见 IDR/crates/idr-store/src/lib.rs:359-387，关键行为在 :371-374。

Store 的身份空间实际是 (kind, contract_id)：idr-store/src/lib.rs:162-169,446-449。不同 Kind 可以合法拥有相同 UUID。

可复现方式

构造上游 Intent，contract ID = X；

构造依赖该 Intent 的下游 Response/Action，其 contract ID 也人为设置为 X，但 Kind 不同；

提交上游 successor；

失效遍历遇到下游 pointer 时，只因 contract_id == X 就跳过；

下游旧合约不会进入 invalidated_refs。

正常构造器随机 mint UUID 很难碰撞，但 wire authoritative contracts 仍可反序列化，攻击者可主动选择相同 ID 并重算 digest，因此这是可利用的信任边界漏洞。

推荐修复

只跳过同一 lineage：

pointer.kind() == stale.kind()
    && pointer.contract_id() == stale.contract_id()

更稳妥的是跳过当前正在替换的精确 aggregate key，而不是单独比较 UUID。加入跨 Kind 同 UUID 的回归和 property test。

六、P1 问题

P1-01：Proof verifier 的“expected binding”不完整

verify_now/verify_at 只接收 expected kind、subject digest、tenant：trust.rs:322-384。虽然 signed claims 包含 subject ref、scope、purpose、policy revision，但通用 verifier 不要求调用方给出这些 expected values。注释声称 purpose binding 已完成，与 API 实际保障不一致：trust.rs:387-389。

修复： verifier 必须消费结构化 ExpectedProofBindingV1，逐项匹配 kind、issuer、subject ref/digest、tenant、scope、purpose、policy revision；Proof ID/nonce 进入持久 replay index。

P1-02：Proof Trust Root 和可信时间仍由普通调用者提供

ProofTrustRootV1::new 接受调用方传入的 key policies：trust.rs:302-320；verify_now 使用系统 wall clock：trust.rs:322-340。若生产组件可任意建立自己的 Trust Root，就可以把攻击者 key 当作 pinned key。

修复： Trust Root 必须来自受保护配置/密钥服务，构造入口不对业务代码开放；使用可注入但受治理的 trusted time source，记录 clock epoch/skew policy。

P1-03：Decision 的 selected option 没有语义绑定 Action effect

Decision 只验证 selected option ref 属于 options：decision.rs:282-307。Store 校验 Action authority boundary，但没有证明 Action operation/parameters 正是 selected option 所描述的 effect：idr-store/src/lib.rs:683-713。

修复： Decision Option 增加 action template/effect digest；Action 引用 selected_option_ref 并验证 operation、parameter constraints、impact 和 compensation。

P1-04：effective_value() 会返回 Rejected/Expired/Superseded Assertion 的值

effective_value() 只选择 corrected value 或原值：human_model.rs:323-327。它不检查 lifecycle status、correction state、expiration 或 usage policy。被拒绝、过期或已替代的 Assertion 仍会暴露可消费值。

修复： 改为：

fn effective_value_at(&self, context, now) -> Result<Option<&Value>, UsageError>

只有 Active、未过期、scope/usage/impact 允许时返回；Rejected/Expired/Superseded 返回 None。

P1-05：同一 Contract lineage 被强制固定在同一 Turn

lineage_context_is_continuous 要求 previous 和 next 的 run_id、turn_id、Authority Context 全相同：idr-store/src/lib.rs:626-633。

这能防止权限漂移，但也会阻止用户在下一 Turn 补充信息后修订同一 Decision/Intent aggregate，与设计中的 WAITING_INPUT→新事实→Contract Revision 不完全兼容。

修复： tenant/subject/authority lineage 必须连续；run/turn 可按明确 revision policy 变化，并保存 causation event、new turn ref 和 reauthorization requirement。

P1-06：Fast Path Block 被标记为 Run Succeeded

Runtime 在 Fast Path Block 时返回 action_posture=Blocked，但 next_run_state=Succeeded：idr-runtime/src/lib.rs:66-74。

这会把政策拒绝、权限拒绝和正常成功混为一类，影响审计、重试、指标和用户体验。

修复： 增加 Blocked/Rejected run state 或用 terminal reason contract；至少不要把拒绝记为 Succeeded。

P1-07：跨语言 UUID 和 canonical encoding 仍存在漂移

Rust 使用 uuid serde 并序列化为 canonical hyphenated lowercase：common.rs:14-59；

TypeScript 接受 uppercase UUID 并原样返回：packages/interaction-client/src/index.ts:482-495；

Python 解析后返回 str(parsed)，会规范化为 lowercase：research/evaluation/src/idr_eval/contracts.py:135-149。

此外 Contract digest 仍依赖 Rust serde_json 序列化，而没有冻结 RFC 8785/JCS 或等价跨语言规则。Proof canonical bytes 也只有 Rust 实现，没有跨语言 golden vectors。

修复： 定义 IDR Canonical Encoding V1；所有语言输入后统一规范化 UUID；发布 bytes + digest + signature golden vectors。

P1-08：Store 仍是整文件 snapshot，缺少 WAL、outbox 和多实例生产一致性

单节点文件锁、rename 和 fsync 已改善，但每次提交重写完整 snapshot：idr-store/src/lib.rs:795-987。它没有事务日志、业务执行 outbox、数据库级 CAS、分布式 ownership 或故障注入矩阵。

修复： foundation store 保留测试用途；生产实现迁移到 append-only ledger + materialized current projection + transactional outbox。

P1-09：时间窗口边界语义不一致

Proof：valid_at >= expires_at 无效：trust.rs:362-365；

Authorization：valid_at > expires_at 才无效，即恰好 expires_at 仍有效：action.rs:258-262；

Response：valid_at > expires_at 才无效：response.rs:335-338。

修复： 全协议统一半开区间 [issued_at, expires_at)，并提供一个共享 time-window newtype/validator。

P1-10：测试没有覆盖本轮新增安全原语的关键攻击路径

当前缺少至少以下测试：

Deny Authorization 生成 Execution Permit；

Permit lease 超过 Authorization expiry；

同一 Proof 派生不同 provider/owner/attempt/nonce；

Proof nonce replay；

expected scope/purpose/policy mismatch；

cross-kind same UUID invalidation；

claim 后 crash/takeover/reconcile；

Failed Receipt 后合法 retry；

Receipt provider/attempt/permit mismatch；

Human Model Gate Decision 重用；

Rejected/Expired Assertion 的 effective_value；

forged Outcome Observation；

snapshot+anchor 同时回滚；

orchestrator CAS、timeout、cancel、invalidation 和 restart recovery。

七、P2 问题

P2-01：协议版本仍只有常量，没有 reader/writer compatibility 与 migration

当前主要使用 schema_version=1、rule_version=1，没有 minimum reader version、feature negotiation、migration registry 和 deprecation policy。

P2-02：全局资源预算仍不完整

Action parameters/Execution result 已有 1 MiB、32 层、10,000 nodes，文本集合也有限制：common.rs:404-455。但 Store snapshot 文件、record 总量、claim 总量、orchestrator steps/dependencies、proof key policy 数量仍缺少统一预算。

P2-03：Orchestrator record 信息不足

Step record 没有 created/updated/deadline、last error、output contract refs、policy revision、owner lease、result digest；Run record 是公开字段且没有完整 validation/transition API：orchestrator.rs:99-265。

P2-04：ActThenRespond 已成为不可由 selector 产生的死模式

Enum 仍有 ActThenRespond：guards.rs:192-202，但 selector 强制 response planned 且默认返回 RespondThenAct：guards.rs:204-228。Turn Contract 仍保留 ActThenRespond 校验：turn.rs:205-214。

应明确删除、保留为未来版本，或提供明确选择条件，避免协议表面与 Runtime 实际能力漂移。

P2-05：包完整性清单未签名

SHA256SUMS 能检测传输损坏，但不能证明发布者身份。正式审计/发布包应使用 release signing key、Sigstore 或受保护 CI attestation。

P2-06：共享 fixture 覆盖面仍过窄

当前跨语言验证主要围绕 supplier confirmation fixture。需要扩展：Proof signing、Authorization、Permit、Response policy、Receipt、Outcome、Human Model、revision invalidation、malformed wire 和 version migration fixture。

八、Aegis Life 适配器专项结论

已正确实现

IDR core 没有依赖 Aegis types；

Aegis adapter 是独立 crate；

host authority 被翻译为 HostAuthorityCandidateV1，不是 IDR Authority Context；

adapter 不依赖 store、不执行动作、不创建 Receipt/Human Model；

当前只暴露 assess_shadow：Aegis-Life/crates/idr-aegis-adapter/src/lib.rs:81-85。

仍需防止

assess_shadow 接受完整 InteractionAssessmentRequestV1，而其中 facts 仍由 Host 提供。因此未来生产 adapter 不能直接沿用该 API；必须先经过 IDR Gateway、Context/Authority Runtime 和 Proof verification。

生产适配器必须满足权限单调性：

IDR granted scope ⊆ host requested scope
IDR granted operation ⊆ host requested operation
IDR granted tenant/subject == verified host identity

九、推荐修复顺序

阶段 0：保持全部生产 Gate 为 BLOCKED

Round 3 不具备解除任何生产 Gate 的条件。

阶段 1：立即修复新增安全原语自身缺陷

Permit 必须拒绝 Deny Authorization；

Permit lease 不得超过 Authorization expiry；

Proof 必须签名完整 Permit request；

Proof nonce 原子消费；

修复跨 Kind 同 UUID 失效绕过；

添加对应攻击测试。

这是最高优先级，因为当前新增类型如果被误接入，会制造新的虚假安全感。

阶段 2：封闭权威对象颁发路径

Contract wire candidate 与 authoritative object 分离；

所有 authoritative constructors crate-private；

authoritative objects 不可 Deserialize；

Store 只接受 signed/sealed Runtime envelope。

阶段 3：把 Proof 接入所有 Admission

顺序：

InputAdmission
→ Context/Authority Proof
→ Intent Fast Path Proof
→ Decision Necessity Proof
→ Turn Planning Proof
→ Response Policy Proof / Send Permit
→ Action Admission Proof / Execution Permit

阶段 4：重做 Execution 状态机

durable lease/owner/attempt；

dispatch/reconcile/takeover；

provider-signed receipt；

multi-attempt receipts；

compensation attempt；

crash injection tests。

阶段 5：实现真实 Orchestration Runtime

实现 repository、scheduler、event loop、wait/wakeup、timeout、retry、cancel、invalidation/recomposition、outbox。

阶段 6：闭合 Outcome 与 Human Model

Signed Outcome Observation
→ Attribution Review
→ Human Model Candidate
→ Verified Promotion Evidence
→ Runtime-issued Assertion

阶段 7：生产 Store/Audit Ledger

append-only database event log；

hash chain；

materialized projections；

transactional outbox；

external WORM/transparency checkpoint；

multi-instance CAS。

阶段 8：跨语言规范与发布门禁

single schema source；

canonical encoding；

proof/signature golden vectors；

fuzz/property tests；

signed release artifacts；

Aegis privilege monotonicity integration tests。

十、生产验收门槛

只有以下条件全部满足，才可重新讨论生产 Trust Root：

P0 = 0
Authoritative contract path sealed = PASS
All admissions require verified proof = PASS
Input provenance + replay protection = PASS
Authorization signature/revocation/trusted time = PASS
Execution permit attack suite = PASS
Claim crash/recovery/retry suite = PASS
Provider-signed receipt = PASS
Outcome observation attestation = PASS
Human Model candidate/promotion chain = PASS
Durable orchestrator restart suite = PASS
External anti-rollback checkpoint = PASS
Cross-language canonical vectors = PASS
Aegis privilege monotonicity = PASS
Independent Rust/Clippy/fmt/Aegis tests = PASS

十一、最终判断

Round 3 已从“协议合约和局部门禁基线”向“信任证明与流程控制基础”前进了一步。其积极之处是：维护者没有把新类型包装成已经完成的生产闭环，并持续固定 production gates 为 blocked。

但从严格架构和安全审计角度，当前仍是：

一套结构越来越完整的 foundation，而不是唯一权威、不可绕过、可恢复、可证明的 Human-Centered Intent & Decision Runtime。

下一轮不应继续横向增加更多 Contract。应集中完成三件事：

封闭权威颁发路径；

让 Proof/Permit 成为所有后继状态变化的强制、单次、持久前置证明；

实现真正的 Orchestrator + Execution lifecycle + immutable Audit Ledger。
