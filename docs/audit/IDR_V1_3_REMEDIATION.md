# IDR V1.3 审计整改台账

审计基线：2026-07-28 外部 GPT 审计  
系统全名：The Human-Centered Intent & Decision Runtime  
简称：IDR

## 当前发布结论

`PRODUCTION TRUST ROOT = BLOCKED`

## 2026-07-29 Round 6 Trust-Chain Closure 候选

当前代码已新增独立的 production path：

- public Candidate / signed Proof 与 sealed Authoritative Record 分离；
- `IdrOrchestratorV1::handle` 成为权威写入口；
- PostgreSQL `SERIALIZABLE` 事务内使用 DB time、当前 Trust Root、proof/nonce
  防重放、aggregate CAS，并原子写 event/snapshot/dependency/audit/outbox；
- Action Admission 成为 exact authoritative record，Reservation 通过外键和 trigger
  只能引用该 Admission；
- Execution、Response、Outcome、Human Model 和 external anchor rollback 均有动态
  PostgreSQL 测试；
- Rust 生成 JSON Schema、TypeScript/Python DTO 及三语言 canonical/digest/signature
  golden vectors；
- file store 被隔离到 `dev-file-store`，与 `production` 同时启用会编译失败；
- Aegis 生产 adapter 独立编译阻断。

完整证据与剩余风险见 `docs/trust-chain-closure/`。由于 production durable
external anchor、checkpoint signing、部署级 Trust Root/Capability/Policy 配置、
provider-facing signed Permit、Response delivery Receipt 等尚未完成，且尚未取得连续
两轮独立无 P0 复审，本文件的 BLOCKED 结论不得解除。

本轮修复已经关闭一组结构完整性、反序列化、时效、重放和跨对象一致性问题，
并完成了 Execution Permit 完整签名绑定、Action-level 唯一 Reservation、Store
可信时钟复核、派发前恢复、provider-signed Receipt 与连续 retry attempt；但尚未
封闭权威 Contract 颁发路径，也未完成所有 admission、Outcome、Human Model、
durable Orchestrator 和外部审计账本的证明链。因此 IDR 仍只能作为
foundation / shadow-mode 验证内核，不能作为生产授权或执行信任根。

## 整改状态

| 审计项 | 状态 | 当前证据与剩余条件 |
| --- | --- | --- |
| P0-01 审计证据包不完整 | 已关闭（仓库侧） | 已提供完整审计清单；外部复审必须按清单提交完整仓库证据，不能只提交协议片段。 |
| P0-02 Guard 结果可被普通对象伪造 | 未关闭 | Human Model 晋级决定已变为 Rust 内不可直接构造，但 Intent、Decision、Action、Response 的上游 facts 仍未由签名证明承载。 |
| P0-03 Exact Authorization 未证明真实权限 | 部分关闭 | 授权现绑定精确 Action、参数摘要、actor、scope 和 Authority Context 摘要；已加入 Ed25519 proof verifier、pinned issuer key policy、用途类型、tenant、时效与撤销校验，但 Exact Authorization 尚未改为只能由 proof 产生。 |
| P0-04 Action 过期与授权重放 | 部分关闭 | Action metadata 过期已强制拒绝；store 增加持久 execution claim、跨实例文件锁和重启后防重放；Receipt 无 claim 不能持久化。仍需让真实执行器只接受不可伪造的 claim/permit。 |
| P0-05 Human Model 可直接晋级 | 部分关闭 | `HumanModelAssertionV1::issue` 必须消费不可直接构造的 gate decision，且 assertion subject 必须等于 Authority Context subject；仍需让 gate facts 由用户确认或 Outcome 证明签名产生。 |
| P0-06 Response 只校验摘要格式和布尔值 | 部分关闭 | Rust 在发送 admission 时重新消费实际 rendered bytes，并检查调用时刻处于 policy 窗口；policy evaluation 绑定 Response、内容摘要和 policy revision。调用时刻仍不是可信时钟，且仍需签名 proof 和唯一发送 façade。 |
| P0-07 Receipt / Outcome 跨对象一致性不足 | 已关闭（store 边界） | commit 与 replay 均复核 Decision→Intent、Action→Decision、Receipt→Action、Outcome→Action/Receipt 和同一 authority context。 |
| P0-08 Metadata 缺少 Contract Kind | 已关闭 | Metadata 内嵌 kind；各合约校验自己的 kind；successor 必须同 kind；store pointer 使用 `(kind, contract_id)`。 |
| P0-09 Serde 绕过受约束构造器 | 已关闭（已识别类型） | Reference、UUID ID、Basis Points 使用 validated custom Deserialize；增加非法 wire value 攻击测试。 |
| P1 Decision→Action 边界失配 | 部分关闭 | 所有不可逆 Action 以及 High/Critical Action 都必须引用 Decision；store 校验 authority refs、自动影响上限、强制确认和禁止自动操作。Decision 选中项与 Action 具体 effect 的语义绑定仍未完成。 |
| P1 Turn 依赖环与顺序失配 | 已关闭 | 校验 DAG、模式顺序路径、每个 Action 的确认 gate，以及授权 gate 只能指向 Action。 |
| P1 currentness 依赖调用方布尔值 | 部分关闭 | durable store 对精确依赖 current pointer 和 invalidation 进行复核；纯函数 facts 仍需证明化。 |
| P1 非规范摘要 | 部分关闭 | wire digest 只接受小写十六进制；尚需冻结跨语言 RFC 8785/JCS 或等价 canonical encoding。 |
| P1 Input actor/role 任意组合 | 已关闭 | Rust、TypeScript、Python 共享 actor-role 允许矩阵并限制 role 数量。 |
| P1 durable store 并发竞争 | 部分关闭 | 增加跨实例文件锁、锁后重读、原子替换、fsync 和并发 claim 测试；尚未实现 WAL、故障注入和多机一致性。 |
| P2 collection budgets | 部分关闭 | 输入 roles、引用、依赖、文本集合和关键模型集合已有上限；Action parameters 与 Execution result 增加 1 MiB、32 层、10,000 nodes 限制。仍需全协议统一预算。 |
| P2 文档漂移 | 本轮更新 | 文档不再把 foundation slice 描述为已获生产信任根资格。 |

## 2026-07-28 第二次复审响应

复审证据：

- `docs/audit/reviews/2026-07-28-reaudit/IDR-V1.3-reaudit-report-2026-07-28.md`
- `docs/audit/reviews/2026-07-28-reaudit/idr-v13-reaudit-test-output.txt`

| 复审问题 | 当前状态 | 本轮实现与仍缺条件 |
| --- | --- | --- |
| P0-01 权威 Contract 创建与持久化路径未封闭 | 未关闭 | authoritative Contract 仍可反序列化，Store 仍接受完整 Contract；必须分离 Candidate、Stored Envelope 和 Runtime-issued sealed commit。 |
| P0-02 Guard facts 无 provenance | 部分基础 | 新增 `ProofKindV1`、`SignedProofEnvelopeV1` 和不可反序列化的 `VerifiedProofV1`；现有 Guard/Admission 尚未全部改为只消费 verified proof。 |
| P0-03 Canonical Input 无来源证明 | 部分基础 | Proof Framework 已支持 `InputAdmission` 类型及 canonical signing bytes；Gateway、session/channel binding 和 nonce replay index 尚未接入。 |
| P0-04 Authorization/Action Admission 无可信 proof | 部分基础 | Ed25519 verifier 校验 pinned issuer/key、proof kind、tenant、subject digest、时效与撤销；Action Admission 仍使用裸 facts。 |
| P0-05 Response policy/time 可伪造 | 未关闭 | 实际 rendered bytes 检查保留；Response facts、可信时钟、单次 send permit 和发送回执仍缺失。 |
| P0-06 Human Model wire 晋级绕过 | 未关闭 | 新增权威 `effective_value(trusted_now)` 生命周期消费语义，但 Candidate→Promotion→Assertion proof 链及 Store 复核尚未实现。 |
| P0-07 claim 无生命周期 | 部分基础 | Store 现可原子消费完整签名 Permit Request 的 Proof ID/nonce 并签发不可构造的 `ExecutionPermitV1`；claim 的 renew/reconcile/takeover 状态机仍未接入。 |
| P0-08 Receipt 无 permit/provider proof | 大部分关闭（Round 5 单节点边界） | Receipt 必须逐字段匹配 dispatched Permit，并消费 provider Ed25519 proof；受保护 provider issuer 配置与在线撤销仍未完成。 |
| P0-09 anti-rollback 仅本地双文件 | 未关闭 | 本地 anchor 只允许截断最后一个无换行 commit marker 的未完成记录；snapshot+anchor 同时回滚仍必须由外部 WORM/透明日志解决。 |
| P0-10 Orchestration Runtime 缺失 | 部分基础 | 新增 Run/Step、Wait、Retry、Command、CAS repository 接口及状态迁移；尚无持久 repository、scheduler 或 event loop。 |
| P0-11 Outcome observation 可伪造 | 未关闭 | Proof kind 已预留 `OutcomeObservation`；Observation Attestation、Attribution Review 和 Human Model Candidate 链尚未实现。 |
| P1-01 Assessment 无法表示为 Turn Plan | 已关闭 | 所有 V1 Turn 必须计划 Response；selector 与 runtime 都拒绝 `response_planned=false`，并增加回归测试。 |
| P1-02 Low 不可逆 Action 跳过 Decision | 已关闭 | 任何 impact level 的不可逆 Action 都必须引用 Decision。 |
| P1-05 anchor 半行恢复 | 已关闭（本地后端） | 换行作为 commit marker；只截断最后一个未完成 record，已提交坏行仍返回 `CorruptAnchor`，含两类故障测试。 |
| P1-07 跨语言 drift | 部分关闭 | Rust UUID 限制与 TypeScript/Python 对齐，wire 整数限制到 JS safe integer；TypeScript 与 Python 都拒绝 unknown fields，Python 已校验完整 Canonical Input，actor-role 矩阵一致。单一 schema 生成仍缺失。 |
| P1-09 Aegis Authority 直拷贝 | 已关闭（shadow 边界） | adapter 只生成 `HostAuthorityCandidateV1`，不再返回 IDR Authority Context；删除未使用的 `idr-store` 依赖。生产权限衰减仍由未来 Authority Runtime 完成。 |
| P2-01 TS decision 无运行时校验 | 已关闭 | 授权构造器使用 runtime enum validator。 |
| P2-02 TS Assessment 不查跨字段不变量 | 已关闭（已声明不变量） | 拒绝 Fast Path/Decision 矛盾及 coordination/posture/run-state 不一致。 |
| P2-04 corrected value 消费不明确 | 已关闭 | `HumanModelAssertionV1::effective_value(trusted_now)` 是权威消费入口，并过滤非 Active、到期和 correction-rejected assertion。 |

`ProductionFeatureGatesV1::BLOCKED` 将生产发布、授权、执行、长期 Human Model
写入和 Aegis 生产 adapter 五个能力固定为关闭。新 Proof、Permit 和 Orchestration
接口是后续闭环的基础，不构成解除生产阻断的证据。

## 2026-07-28 完整复审追加整改

| 复审项 | 当前状态 | 本次处理 |
| --- | --- | --- |
| P0-01 Guard facts 可伪造且互相矛盾 | 部分关闭 | Runtime 现在拒绝 impact 不一致、High Action 无授权、无 Action 却要求授权、危险 parallel/stream 组合，以及 Fast Path Allow 与 Decision Required 同时出现。facts provenance proof 仍未实现。 |
| P0-02 Canonical Input 无来源证明 | 未关闭 | actor-role shape 已有限制，但 Gateway signature、session/channel binding、nonce replay index 尚未实现。 |
| P0-03 Authorization 无签名与撤销 | 未关闭 | 精确字段绑定保留；issuer signature、trusted clock、revocation registry 仍是 P0。 |
| P0-04 Response 可伪造/过期重放 | 部分关闭 | admission 重新校验 rendered bytes，并要求 `valid_at` 位于严格非零 policy window；Policy Engine signature 仍未实现。 |
| P0-05 Human Model facts 未绑定候选 | 部分关闭 | 阻止跨 subject assertion；Candidate→Promotion proof chain 仍未实现。 |
| P0-06 删除 invalidation 标记可恢复旧合约 | 已关闭 | replay 按 revision history 和 dependency DAG 重新推导 expected invalidation closure，并要求与 snapshot 完全一致。 |
| P0-07 空文件与旧快照回滚 | 部分关闭 | 已存在空文件判 Corrupt；snapshot 增加 sequence/previous digest，配套本地 append-only anchor 检测单文件回滚和 anchor 丢失。攻击者同时回滚 snapshot 与本地 anchor 仍需外部 durable anchor 解决。 |
| P0-08 同 revision 分叉 | 已关闭 | 唯一键改为 `(kind, contract_id, revision)`；不同 digest 的同 revision 第二记录直接 Corrupt。 |
| P0-09 claim 被误报为已执行 | 已被 Round 5 Reservation 替代 | 旧 claim API 已删除；Reservation 明确记录 PermitIssued、Delivered、Dispatched 和终态，派发前可恢复，派发后必须 reconciliation。renew/takeover 仍未实现。 |
| P0-10 Receipt 未绑定 permit/provider proof | 大部分关闭（Round 5 单节点边界） | Receipt 绑定 Permit/Authorization/provider/owner/attempt/dispatch nonce，并要求 provider 签名 proof；受保护 provider trust-root 配置仍未完成。 |
| P0-11 successor 可跨上下文 | 已关闭 | commit 和 replay 均要求同 lineage 的 run、turn 与完整 Authority Context 不变。 |

## 2026-07-28 Round 3 严格复审响应

复审证据：

- `docs/audit/reviews/2026-07-28-round3/IDR-V1.3-round3-strict-review.md`
- `docs/audit/reviews/2026-07-28-round3/round2-to-round3-file-diff.txt`

| Round 3 问题 | 当前状态 | 本轮实现与仍缺条件 |
| --- | --- | --- |
| P0-05 Permit 接受 Deny 且租期越过授权 | 已关闭（Permit 边界） | `ExecutionPermitRequestV1::new` 与复核路径都要求 `Approve`；lease 必须处于 Authorization、Action 和 Proof 的共同窗口内；协议时间窗口统一为半开区间。 |
| P0-06 Permit 关键字段未被签名且 Proof 可重复派生 | 已关闭（单节点 Store 边界） | Proof subject digest 现在覆盖完整 Permit Request：Action ref、Authorization ID/digest、provider、owner、attempt、lease、nonce、idempotency key、tenant/scope/purpose/policy revision。`VerifiedProofV1` 不再 `Clone`，Store 在锁内原子持久化消费 Proof ID、nonce 和 Authorization ID 后才返回 opaque Permit，重启后重放仍拒绝。 |
| P0-14 跨 Contract Kind 同 UUID 跳过失效 | 已关闭 | 失效遍历只跳过相同 `(kind, contract_id)` lineage；加入不同 Kind 共用 UUID、下游 revision 依赖和重启重放的真实攻击回归测试。 |
| P1-01 Proof expected binding 不完整 | 已关闭（verifier API） | 新增 `ExpectedProofBindingV1`，强制匹配 kind、issuer、subject ref/digest、tenant、scope、purpose、policy revision。 |
| P1-06 Fast Path Block 被标记 Succeeded | 已关闭 | 新增终态 `Rejected`，Rust Runtime 与 TypeScript 跨字段校验都拒绝把 Block 伪装成 Succeeded。 |
| P1-07 TypeScript UUID 不规范化 | 已关闭（UUID 子项） | TypeScript 在验证后统一输出 lowercase UUID，与 Rust/Python 对齐；JCS 和跨语言签名 golden vectors 仍未完成。 |
| P1-09 时间窗口边界不一致 | 已关闭（现有窗口） | Proof、Authorization、Response 及 Action admission 统一采用 `[issued_at, expires_at)` / `valid_at < valid_until`。 |
| P0-01～04、10～14 及 Human Model/Outcome 链 | 未关闭 | 权威对象 sealed issuance、Ingress provenance、所有 facts proof 化、Response send proof、Outcome/Human Model proof chain、真实 Orchestrator、外部 anti-rollback checkpoint 等仍保持生产阻断。 |

## 2026-07-29 Round 4 严格复审响应

Round 4 原始证据随 Round 5 审计包保存，生产结论继续保持
`PRODUCTION TRUST ROOT = BLOCKED`。

| Round 4 问题 | 当前状态 | Round 5 实现与剩余条件 |
| --- | --- | --- |
| P0-05 Permit 与 legacy claim 双生产通道 | 已关闭（Store API） | 删除公开 `claim_action_execution()`；执行资格只能通过签名 Permit 进入单一 Reservation aggregate。裸 `ActionAdmissionFactsV1` 纯函数仍保留用于 foundation assessment，不再能创建 durable execution claim。 |
| P0-06 verify-then-delay TOCTOU | 已关闭（租期/对象时效） | Store 注入 `TrustedClockV1`，在文件锁后读取时间并重新验证 Action、Authorization、Proof 与 lease；新增过期后延迟消费回归测试。受保护 trust-root 配置与在线 revocation distribution 仍属于 P1-01。 |
| P0-07 双 Authorization 双 Permit | 已关闭（单节点 Store） | Reservation 唯一键同时覆盖 Action ref 与 `(tenant, operation, idempotency_key)`；不同 Authorization、Proof 或并发 Store 实例都只能创建一个 attempt。 |
| P0-08 Permit 丢失与无状态 claim | 部分关闭 | Reservation 持久化完整 request、Authorization、签名 envelope、provider、owner、attempt、lease、delivery、dispatch 与 Receipt 状态；派发前可恢复，派发后强制 reconciliation 而不是重新执行；lease renew、owner takeover 与真实 provider reconciliation loop 尚未实现。 |
| P0-09 Receipt 不绑定 Permit/provider | 大部分关闭（单节点 Store） | Receipt 新增 Permit、Authorization、provider、owner、attempt、dispatch nonce 绑定；裸 Receipt commit 被拒绝；必须提交 `VerifiedExecutionReceiptV1` provider Ed25519 proof；按 attempt 保存 Receipt，失败/拒绝/过期允许连续 retry。受保护 provider issuer 配置仍未完成。 |
| P1-02 proof consumption 审计信息不足 | 已关闭（snapshot evidence） | Reservation 保存完整 signed Permit envelope、key、issuer、request、Authorization、provider、owner、attempt 与 lease；Receipt proof envelope 和验证时刻也持久化并在 replay 中复核绑定。 |
| P1-04 Human Model lifecycle 未参与消费 | 已关闭 | `effective_value(trusted_now)` 对 Rejected、Expired、Superseded、correction-rejected 和到期 assertion 返回 `None`。Candidate→Promotion→Assertion proof 链仍是独立 P0。 |
| TypeScript 离线 typecheck 不可复现 | 已关闭（审计包） | TypeScript、`@types/node` 和 `undici-types` 精确 tarball 放入 `vendor/`，空 npm cache 下 `npm ci --offline` 后可独立执行 10 项测试与 `tsc --noEmit`。 |

本轮没有声称关闭 P0-01～04、P0-10～14。Authoritative sealed issuance、
authenticated ingress、proof-only guards、Response send proof、Outcome Observation、
Human Model Promotion、可运行 Orchestrator 与外部 anti-rollback checkpoint 仍是解除
生产阻断的必要条件。

## 本轮新增的安全不变量

1. wire 数据不能构造空 UUID、非法引用或大于 10,000 的 basis points。
2. Metadata 的 kind 必须与实际 contract kind 一致。
3. successor 必须继承同 kind、同 contract ID 和精确 predecessor digest。
4. Action 在 metadata `valid_until` 后必定拒绝，即使授权仍未过期。
5. 同一 Action、授权 ID 或 `(tenant, operation, idempotency key)` 只能获得一次持久执行 claim。
6. 独立打开同一 store 的两个实例不能同时 claim 同一 Action。
7. Human Model assertion 的 epistemic status 不能由调用方直接传入。
8. Response 内容摘要由 Rust 从渲染内容计算，policy evaluation 必须绑定该精确摘要。
9. Tool、Agent、Scheduler 等 actor 不能伪装成不允许的 semantic role。
10. Turn dependency graph 必须无环，确认 gate 必须逐个约束 Action。
11. Execution Receipt 没有对应的 dispatched Reservation 与 provider 签名 proof 时不能进入 store。
12. replay 必须重新推导失效闭包；删除持久化 `invalidated_refs` 会被判为损坏。
13. 已存在空 snapshot、同 revision 分叉和跨上下文 successor 都会被拒绝。
14. snapshot sequence/digest 与本地 append-only anchor 不一致时会报告 rollback。
15. dispatched 但无 Receipt 的执行不得重新签发 Permit，必须进入 reconciliation。
16. Response admission 必须拿到实际 rendered bytes 和当前发送时刻。
17. Decision 的 `UserDecided` 阶段必须记录 `selected_option_ref`；启用权重时所有
    criteria 权重必须完整且总和为 10,000 basis points。
18. V1 Runtime 不得返回无法构造成 Turn Plan 的 `response_planned=false` 状态。
19. 任何不可逆 Action 都必须依赖 Decision。
20. anchor 只能恢复最后一个未提交尾部；已提交坏行不得被静默截断。
21. 只有 Ed25519 签名、pinned issuer policy，以及完整 expected kind、issuer、
    subject ref/digest、tenant、scope、purpose、policy revision、时间和撤销全部通过，
    才能产生不可反序列化且不可 Clone 的 `VerifiedProofV1`。
22. Execution Permit Proof 必须签名完整 `ExecutionPermitRequestV1`；Store 必须原子
    消费 Proof ID、nonce 与 Authorization ID 后才能签发不可公开构造的
    `ExecutionPermitV1`。
23. Aegis host Authority 只能进入 candidate 边界，不能直接升级为 IDR grant。
24. 失效传播只能跳过相同 `(contract kind, contract ID)` lineage，不能仅凭 UUID 跳过。
25. Proof、Authorization、Response 与 Action 的到期边界统一为半开区间。
26. Fast Path Block 的终态是 `Rejected`，不得记为 `Succeeded`。
27. 同一 Action ref 或同一 `(tenant, operation, idempotency_key)` 只能存在一个
    durable Execution Reservation；不同 Authorization 不能生成并行 Permit。
28. Store 必须在文件锁内读取 `TrustedClockV1` 并重新验证 Action、Authorization、
    signed Proof 与 Permit lease；验证后延迟到过期再消费必定失败。
29. Permit 的完整 signed envelope、request、Authorization、provider、owner、
    attempt、lease 和状态必须先持久化，才能返回 executor-facing Permit。
30. Permit 在 provider dispatch 前可以由同一 owner 恢复；dispatch 后不得重发，
    必须进入 reconciliation。
31. Receipt 必须逐字段匹配已派发 Permit，并消费 provider Ed25519 proof；裸 Receipt
    不能通过通用 commit 进入 Store。
32. failed/rejected/expired attempt 只能以同一 Reservation 的下一个连续 attempt
    编号重试；成功、补偿或部分成功不能自动重试。
33. Human Model assertion 只有 Active、未到期且未被 correction reject 时才提供
    effective value。

## 解除 BLOCKED 的必要工作

1. 将现有通用签名 Proof Framework 接入 Input、Intent、Action、Response、Outcome
   和 Human Model 的真实生产 admission，并保存 proof dependency。
2. 将 pinned issuer policy 移入受保护配置，接入可信时间、key rotation 和
   revocation distribution；把当前 Execution Permit replay index 扩展到所有 Proof。
3. 将 Action / Response / Human Model admission 改为只消费 `VerifiedProofV1`，
   移除生产路径对裸布尔 facts 的依赖。
4. 将当前 Store 原子 Permit 签发接入完整 claim lease/owner/attempt/recovery 状态机；
   Executor 只能消费 Permit，Receipt 必须绑定 Permit 并验证 provider signature。
5. 冻结语言中立 schema、canonical encoding、digest/signature test vectors，
   并在 Rust、TypeScript、Python 中做一致性测试。
6. 增加进程崩溃、断电、锁竞争、部分写入、撤销竞态和授权重放攻击测试。
7. 完成上述项目后重新进行独立 trust-root 审计；在此之前不得修改本文件的
   `BLOCKED` 结论。
