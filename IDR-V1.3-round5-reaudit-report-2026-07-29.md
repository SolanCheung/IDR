# The Human-Centered Intent & Decision Runtime V1.3
## Round 5 严格复审报告

审计对象：`IDR-V1.3-reaudit-round5-2026-07-29.zip`  
审计日期：2026-07-29  
系统正式名称：**The Human-Centered Intent & Decision Runtime（IDR）**  
审计基准：`source-design/IDR-V1.3-original-design.txt`、Round 4 审计问题、Round 5 全量源码、测试与整改说明  
压缩包 SHA-256：`688296699de524e25dc40055a26a96bbd0ba41780c77e560f0db02a22d561619`

---

# 一、总体结论

```text
PRODUCTION TRUST ROOT = BLOCKED
FOUNDATION / SHADOW MODE = ACCEPTABLE WITH LIMITATIONS
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

Round 5 是一次明确且有效的整改，不是只增加文档或类型。上一轮关于执行资格分裂、Permit 消费时间竞态、Receipt 与 Permit 脱节的问题，已有多项被实质关闭：

- 删除了公开的 legacy `claim_action_execution()` 生产路径；
- 建立了以 `Action ref` 和 `(tenant, operation, idempotency_key)` 为唯一约束的持久 `ExecutionReservation`；
- Store 在文件锁内读取可信时间，再验证 Action、Authorization、Proof 和 lease；
- 两个不同 Authorization 不再能为同一 Action 并发签发两份 attempt-1 Permit；
- Permit 请求、Authorization、Proof envelope、provider、owner、attempt、lease、nonce 和状态均被持久化；
- 增加 `PermitIssued → Delivered → Dispatched → terminal` 状态；
- Receipt 精确绑定 Permit ID、Authorization ID、provider、owner、attempt 和 dispatch nonce；
- Store 拒绝裸 Receipt，要求 Ed25519 Provider Proof；
- 正常失败、拒绝和已显式过期的 attempt 可以进入连续重试；
- Human Model 消费侧新增 `effective_value(trusted_now)` 生命周期过滤；
- TypeScript 审计依赖已完整随包提供，可独立离线安装和 typecheck。

这些都是重要进展。Round 5 已从“Permit 与 claim 双通道”推进到“单一 Reservation 基础状态机”。

但它仍未形成完整、不可绕过、可撤销、可恢复的生产信任链。当前最严重的剩余问题已经集中到以下三类：

1. **执行资格在签发后没有持续绑定 Action 的当前性。**  
   Action 因上游修订而递归失效后，已经签发的 Permit 仍可进入 `Delivered` 和 `Dispatched`，从而执行一个已经失效的行动。

2. **Reservation 的故障恢复状态机仍存在永久死锁。**  
   Permit 在返回调用者前丢失，且第一次恢复发生在 lease 到期后时，`recover_execution_permit()` 只返回错误，不把 attempt 持久化为 `Expired`；后续新 attempt 又因旧状态仍是 `PermitIssued/Delivered` 而永久被拒绝。

3. **密码学 Proof 尚未成为端到端治理链。**  
   Authorization、Input、Intent/Decision/Turn facts、Action Admission、Response Admission、Outcome 和 Human Model 仍有普通对象或裸事实入口；Store 消费 Proof 时也不持有当前 Trust Root，无法在线复查密钥撤销。

因此，Round 5 可以继续作为：

- Rust 协议与单节点 Store 基线；
- Execution Reservation 原型；
- Aegis Life shadow-mode 验证宿主；
- 跨语言边界与离线评估基础。

但不得被认定为完整的生产 Trust Root，也不得授予真实生产 Authorization、不可逆 Execution 或长期 Human Model 写入。

本轮确认：

- **P0：14 项**
- **P1：10 项**
- **P2：6 项**

其中“14 项”是审计分组方式；若把相关问题合并，数量可以减少，但 `BLOCKED` 结论不受影响。

---

# 二、审计范围与独立验证

## 2.1 压缩包安全与完整性

- ZIP 条目：471
- 解压前路径穿越检查：0
- 符号链接：0
- 打包时有效文件：345（含 `SHA256SUMS` 自身）
- `SHA256SUMS` 清单记录：344
- 清单验证：**344/344 PASS**
- Round 4 → Round 5：新增 6、删除 0、修改 14

注意：本轮最初进度消息把“总文件数”和“清单记录数”混写成了 345/345。准确口径是：**包内 345 个有效文件，其中 344 个文件列入 `SHA256SUMS`，校验结果 344/344 通过。**

## 2.2 独立执行结果

| 项目 | 独立结果 |
|---|---:|
| TypeScript 离线 `npm ci` | PASS |
| TypeScript Node tests | **10/10 PASS** |
| TypeScript `tsc --noEmit` | PASS |
| Python evaluation tests | **7/7 PASS** |
| ZIP SHA-256 清单 | **344/344 PASS** |
| Rust workspace tests | 未独立执行：当前审计环境没有 `cargo` / `rustc` |
| Aegis Rust tests | 未独立执行：当前审计环境没有 `cargo` / `rustc` |

维护者提供的 `VALIDATION-SUMMARY.md` 声明：

- IDR Rust tests：41 passed；
- strict Clippy：PASS；
- rustfmt：PASS；
- Aegis adapter tests：2 passed；
- Aegis workspace check：PASS。

这些记录与代码结构没有明显冲突，但仍属于维护者验证证据，不能替代本轮未能执行的独立 Rust 动态验证。

---

# 三、Round 4 问题关闭情况

| Round 4 重点问题 | Round 5 结论 |
|---|---|
| Permit 与 legacy claim 双通道 | **已关闭**：公开 legacy claim 已删除 |
| 同一 Action 可由不同 Authorization 取得多个 attempt-1 Permit | **已关闭**：Action 与 idempotency 双重 Reservation 唯一性 |
| Permit 消费时未在 Store 锁内复核当前时间 | **已关闭（Permit issuance）** |
| Permit/Authorization/Proof 未完整持久化 | **已关闭** |
| Permit 丢失前没有恢复入口 | **部分关闭**：增加 recovery，但过期恢复存在永久死锁，见 P0-08 |
| Receipt 不绑定 Permit/provider/attempt/nonce | **显著关闭** |
| Store 接受裸 Receipt | **已关闭** |
| 失败 attempt 后无法创建下一 attempt | **正常 Failed/Rejected/Expired 路径已关闭** |
| Human Model 过期/拒绝值仍可被消费 | **局部关闭**：新增 `effective_value` |
| Permit 签发后 Action 失效 | **未关闭；Round 5 新暴露的关键 P0** |
| Trust Root 在线撤销复核 | **未关闭** |
| Action Admission 与 Permit 强绑定 | **未关闭** |
| Runtime sealed authoritative issuance | **未关闭** |
| Input/Response/Outcome/Human Model proof chain | **未关闭** |
| Durable Orchestrator | **未关闭** |
| 外部 anti-rollback | **未关闭** |

Round 5 解决的是**执行资格的唯一预留和正常路径状态记录**，还没有解决**执行资格的持续有效性、故障恢复完备性和上游证明链**。

---

# 四、原始设计与实现逐项对应

原始设计要求 IDR 不只是 Contract 库，而是贯穿输入、意图、决策、响应、授权、执行、结果和 Human Model 的治理 Runtime。关键设计段落包括：

- `source-design/IDR-V1.3-original-design.txt:515-625`：Intent Fast Path；
- `...:626-735`：Decision Necessity；
- `...:873-910`：Action Admission；
- `...:911-971`：Execution Receipt 与 Outcome；
- `...:972-1009`：Human Model Update Gate；
- `...:1010-1043`：Orchestration Runtime；
- `...:1044+`：Audit Ledger 与治理平面。

| 原始设计能力 | Round 5 实现 | 复审结论 |
|---|---|---|
| Canonical Input | Rust/TS/Python shape、actor-role matrix、unknown-field 拒绝 | 结构较强，来源认证和 nonce 消费缺失 |
| Intent Fast Path | Rust 确定性 guard + runtime assessment | 条件正确，facts provenance 缺失 |
| Decision Necessity | Rust 确定性 guard | 条件正确，facts provenance 缺失 |
| Intent / Decision Contract | 完整类型、摘要、依赖、版本 | authoritative issuance 未封闭 |
| Turn Coordination | mode、DAG、gate、顺序校验 | 类型较强，仍由 caller facts 驱动 |
| Response Admission | 实际 bytes 摘要、policy window | 内容绑定正确，Policy Proof/可信发送缺失 |
| Action Admission | Contract + Authorization + pure facts gate | 未成为 Permit issuance 的强制前置证明 |
| Execution Permit | 签名请求、锁内可信时间、唯一 Reservation | 显著增强；失效联动和恢复仍不完备 |
| Execution Receipt | Permit/provider/attempt/nonce 绑定和 Provider Proof | 显著增强；Proof context 与锁内当前性仍有缺口 |
| Outcome | 独立 Contract、Action/Receipt 一致性 | 没有 Observation Attestation |
| Human Model | Gate、Assertion、effective value | Promotion chain 未绑定内容和真实证据 |
| Durable Store | 锁、原子替换、fsync、重放、失效重算、Reservation | 单节点基础强；无 WAL/outbox/外部 anchor |
| Orchestration Runtime | step/run/command/CAS trait | 仍是协议表面，不是运行控制平面 |
| Aegis adapter | Host Authority Candidate、shadow assessment | 反向耦合受控；无生产 Adapter |

---

# 五、语言职责边界

## 5.1 Rust

Rust 确实承担了 Contract、Guard、Proof、Permit、Store 和 Orchestration 类型，符合“Rust 是生产信任根”的语言分工方向。

但当前缺失的是 Rust 内部的第二道边界：

```text
不可信 Rust Host / Adapter / Library caller
          ≠
IDR Runtime authoritative issuer
```

只要 authoritative Contract 仍能被公开 `issue()` 或 wire `Deserialize` 直接创建，并由 Store 接受，“使用 Rust”本身不能证明对象由可信 Runtime 颁发。

## 5.2 TypeScript

TypeScript 仍限定为：

- transport shape 验证；
- Canonical Input 提交；
- Runtime assessment 解析；
- exact Action authorization submission 构造。

没有复制 Rust 的 Fast Path、Decision Necessity 或 Action Admission 逻辑。独立 `npm ci --offline`、10 项测试和 typecheck 均通过。职责边界总体正确。

## 5.3 Python

Python 仍只做离线 fixture evaluation，没有发现生产授权、Contract 写入、Execution 或 Human Model 晋级代码。独立测试 7/7 通过。职责边界正确。

## 5.4 Aegis Life

Aegis adapter 继续只把 Host Authority 转换为不可信 Candidate，并运行 shadow assessment，没有直接进入 IDR authority、permit、receipt 或 Human Model 写入链。当前没有明显反向耦合或权限扩大。

但它也仍然只是 reference/shadow adapter，不能作为生产执行边界验收。

---

# 六、已正确实现或显著加强的部分

1. **单一 Execution Reservation 唯一性**  
   `IDR/crates/idr-store/src/lib.rs:453-524` 同时按精确 Action ref 和 `(tenant, operation, idempotency_key)` 查找冲突，并限制 attempt 连续性。

2. **Permit issuance 的锁内可信时间复核**  
   `idr-store/src/lib.rs:422-451` 在获得进程内锁和文件锁、重载 snapshot 后读取 `TrustedClockV1`，再校验 Action currentness 和 verified request。

3. **完整 Permit 请求持久化**  
   `idr-store/src/lib.rs:109-139,483-557` 持久化 Authorization、signed proof envelope、issuer/key、provider、owner、attempt、lease、nonce 和状态。

4. **Permit 请求完整绑定**  
   `idr-protocol/.../execution.rs:23-177` 把 Action、Authorization、provider、owner、attempt、lease、dispatch nonce、idempotency 和 authority context 纳入 request digest 与 expected proof binding。

5. **拒绝 Deny 与越界 lease**  
   `execution.rs:61-76,131-160` 要求 Authorization 为 Approve，并限制 lease 不超过 Authorization 和 Action 有效期。

6. **正常路径状态机存在**  
   `idr-store/src/lib.rs:141-170,635-689` 建立 PermitIssued、Delivered、Dispatched 和 terminal 状态，并持久化 delivery/dispatch 时间。

7. **Provider-signed Receipt**  
   `execution.rs:332-622` 的 Receipt 绑定 Permit ID、Authorization ID、provider、owner、nonce、attempt，且 Store 只接受 `VerifiedExecutionReceiptV1`。

8. **拒绝裸 Receipt**  
   `idr-store/src/lib.rs:265-289` 普通 `commit()` 明确拒绝 Execution Receipt，要求专用 verified receipt 路径。

9. **Receipt 与 Reservation 跨对象一致性增强**  
   `idr-store/src/lib.rs:1226-1265` 检查 reservation、attempt、dispatch、provider、owner、nonce、idempotency、parameter digest 和执行时间。

10. **失败 attempt 的连续 retry 基础**  
    `idr-store/src/lib.rs:155-170,512-524` 允许 Failed、Rejected、Expired 进入下一连续 attempt，并拒绝跳号。

11. **递归失效与同 revision 分叉防护保持有效**  
    Store 会重算 invalidation closure，并按 `(kind, contract_id, revision)` 约束唯一记录。

12. **Human Model 生命周期过滤**  
    `human_model.rs:323-336` 对非 Active、correction rejected、未生效或时间过期 Assertion 返回 `None`。

13. **TypeScript 可复现审计环境**  
    Round 5 随包提供精确审计依赖，`npm ci --offline`、tests 和 typecheck 均可独立完成。

---

# 七、缺失的设计能力

1. Runtime-sealed authoritative Contract envelope；
2. 认证 Gateway、session/channel 绑定与 Input nonce consumption；
3. Guard facts 的 evidence snapshot 与 typed proof；
4. 用户 Action Authorization 的认证签名和在线撤销；
5. Action Admission Proof 到 Execution Permit 的强制依赖；
6. Permit 对 Action currentness/invalidation 的持续检查；
7. 完整 reservation lease expiry、renew、takeover、abandon、reconcile；
8. Store 消费时的当前 Trust Root 与 key revocation 复核；
9. Receipt Proof 的完整 authority-context 绑定和锁内时效复核；
10. Response send permit 与 Delivery Receipt；
11. Outcome Observation Attestation；
12. Human Model Candidate → Promotion Proof → Assertion；
13. 可运行 durable Orchestrator；
14. append-only WAL、transactional outbox、多节点 fencing；
15. 外部 WORM/透明日志 anti-rollback；
16. 语言中立 Contract canonical encoding 和跨语言签名 golden vectors。

---

# 八、P0 问题

## P0-01：权威 Contract 颁发与持久化路径仍未封闭；BLOCKED feature gates 只是声明

**代码证据**

核心 authoritative 类型仍公开 `Deserialize` 和/或 `issue()`：

- `intent.rs:146-190`
- `decision.rs:127-181`
- `turn.rs:51-96`
- `response.rs:48-102`
- `action.rs:23-82,196-240`
- `execution.rs:332-420`
- `outcome.rs:29-87`
- `human_model.rs:147-235`

Store 公开可反序列化的 Contract enum，并接受完整 Contract：

- `idr-store/src/lib.rs:29-81`
- `idr-store/src/lib.rs:265-276`

`ProductionFeatureGatesV1::BLOCKED` 定义于：

- `trust.rs:51-78`

全仓搜索显示它只在文档和单元测试中被引用，没有被 Runtime、Store、Authorization、Permit 或 Human Model 写入入口强制检查。

**可复现方式**

任意链接 IDR crate 的 Rust Host 可直接创建一个摘要一致的 Intent/Decision/Action/Outcome/HumanModelAssertion，并调用 Store `commit()`。过程不需要 Runtime seal，也不触发 `ProductionFeatureGatesV1::BLOCKED`。

**影响**

Rust 是实现语言，但不是封闭发行权限。被攻陷或错误实现的 Host/Adapter 可以制造 authoritative 状态。声明为 BLOCKED 不等于技术上无法调用生产能力。

**推荐修复**

- authoritative `issue()` 改为 `pub(crate)`；
- authoritative 类型不直接 wire `Deserialize`；
- 外部只提交 Candidate/Submission；
- Runtime 产生签名 `SealedContractEnvelopeV1`；
- Store 只接受 sealed envelope；
- Production feature gate 必须由生产 façade 强制检查，而不是仅存在一个常量。

---

## P0-02：Intent Fast Path、Decision Necessity 和 Turn Coordination 仍由调用者控制事实

**代码证据**

- `idr-runtime/src/lib.rs:19-26`：Request 直接携带三组 facts；
- `idr-runtime/src/lib.rs:52-100`：直接调用 pure guard/selector；
- `idr-runtime/src/lib.rs:103-120`：只检查少量结果矛盾，不验证 facts 来源。

**可复现方式**

调用者提交：

```text
deterministic_command_match=true
required_parameters_complete=true
ambiguity_present=false
impact_level=low
reversible=true
```

而无需提交命令匹配证据、Context Snapshot、Authority Proof、Policy Proof 或 Capability Proof。

**影响**

Guard 算法本身正确，但攻击者能够控制算法输入。Fast Path、Decision Necessity 与 Turn Mode 不是 proof-carrying governance node。

**推荐修复**

生产 Runtime 只接受：

```text
VerifiedInputAdmission
VerifiedContextSnapshot
VerifiedIntentFastPathFacts
VerifiedDecisionNecessityFacts
VerifiedTurnCoordinationFacts
```

并把对应 proof ref/digest 绑定到后续 Contract。

---

## P0-03：Canonical Input 仍没有认证来源、会话绑定和防重放

**代码证据**

- `input.rs:41-90`：公开 `Deserialize` 和 `new()`；
- `input.rs:92-167`：只验证结构、actor-role matrix、摘要格式和 logical time。

缺少 Gateway signature、authenticated principal、session/channel、nonce 和 durable replay index。

**可复现方式**

构造一个结构合法的：

```json
{"source_actor":"user","primary_semantic_role":"intent", ...}
```

只要 actor-role 组合允许、摘要格式正确，Rust 无法判断它是否来自真实用户会话。

**影响**

“用户已明确选择”“用户已确认”“用户已纠正 Human Model”等上游事实可以建立在伪造输入上。

**推荐修复**

Gateway 签发 `InputAdmissionAttestationV1`，绑定 session、channel、principal、tenant、event ID、content digest、nonce、received_at；Store/Orchestrator 原子消费 nonce。

---

## P0-04：Exact Action Authorization 仍不是可信用户授权证明

**代码证据**

- `action.rs:196-240`：Authorization 公开 `Deserialize` 和 `issue()`；
- `action.rs:242-292`：精确绑定 Action/context/time，但没有认证 issuer signature、MFA/session context 或在线撤销证明。

**可复现方式**

普通 Rust 调用者直接调用：

```text
ExactActionAuthorizationV1::issue(..., Approve, ...)
```

即可获得结构合法 Authorization，再交给 Permit signer 签发 Execution Permit Proof。

**影响**

Round 5 已证明 Permit signer 对某个 Authorization 签了名，但没有证明该 Authorization 由真实、有权且已认证的用户签发。

**推荐修复**

Authorization 应由 `AuthenticatedUserConfirmationEvent + AuthorityDecisionProof` 共同生成，并由专用 Authorization issuer 签名；Permit 链必须消费 `VerifiedExactAuthorizationV1`，不能只消费普通对象。

---

## P0-05：Action Admission 没有成为 Execution Permit 的强制前置条件

**代码证据**

- `action.rs:294-307`：Action Admission 事实仍是普通 bool；
- `action.rs:327-394`：pure evaluator；
- `execution.rs:51-60`：Permit Request 只要求 Action、Authorization 和执行参数；
- `idr-store/src/lib.rs:407-557`：Permit issuance 验证 Action currentness、Authorization、Proof、时间和 Reservation，但不要求 `ActionAdmissionDecision` 或 Capability/Authority/Policy/Precondition typed proofs。

**可复现方式**

不调用 `evaluate_action_admission()`，直接：

1. 创建 Action；
2. 创建 Approve Authorization；
3. 构造并签名 Execution Permit Request；
4. 调用 `issue_execution_permit()`。

只要 Permit issuer 被 Trust Root 接受，Store 不知道 capability、actor authority、agent authority、precondition、policy、verification 或 compensation 是否真正通过。

**影响**

删除 legacy claim 关闭了双通道，但没有让 Action Admission 成为唯一执行治理节点。Permit signer 实际上可以替代并绕过 Action Admission。

**推荐修复**

`ExecutionPermitRequestV1` 必须绑定一个 `VerifiedActionAdmissionProofV1`，其 subject digest 覆盖：Action、Authorization、Capability、Authority、Policy、Precondition、Verification、Compensation 和 dependency snapshot。Store 必须直接验证并消费该 proof。

---

## P0-06：Store 消费 Permit/Receipt 时不重新验证当前 Trust Root 和密钥撤销

**代码证据**

- `trust.rs:422-467`：只有 `ProofTrustRootV1::verify_at()` 检查签名、key policy、有效期和 `revoked_at`；
- `execution.rs:308-329`：`VerifiedExecutionPermitRequestV1::validate_at()` 只复核对象绑定和时间，不重新验签或读取当前 Trust Root；
- `idr-store/src/lib.rs:407-451`：Store 只接收 opaque verified request，没有 Trust Root；
- `execution.rs:637-647`、`idr-store/src/lib.rs:278-289`：Receipt 同样只复核已验证对象的时间。

**可复现方式**

1. 在密钥未撤销时把 Proof 验证为 `VerifiedProofV1`；
2. 更新生产 Trust Root，把该 key 标记为 revoked；
3. 保留旧的 Verified object；
4. 之后再调用 Store issuance/commit。

Store 没有当前 Trust Root，无法得知撤销已经发生。

**影响**

紧急 key revocation 无法阻止已验证但尚未消费的 Permit/Receipt Proof。撤销语义止于“验证时刻”，没有延伸到“授权消费时刻”。

**推荐修复**

Store/Orchestrator 在持锁事务内使用受保护的当前 Trust Root 对 signed envelope 重新验证；至少检查 current key epoch、revocation generation 和 proof freshness。Verified wrapper 不能被当作永久授权。

---

## P0-07：Action 被递归失效后，已经签发的 Permit 仍可继续 Delivered/Dispatched

**代码证据**

- `idr-store/src/lib.rs:431-451`：只在 Permit issuance 时检查 Action current/not invalidated；
- `idr-store/src/lib.rs:747-779`：上游 successor 只把下游 Contract ref 加入 `invalidated_refs`；
- `idr-store/src/lib.rs:635-689`：Permit transition 只检查 reservation、permit binding、状态和时间，不重新检查 Action currentness/invalidation，也不把失效传播到 reservation。

**可复现方式**

1. 提交 Intent/Decision/Action A；
2. 为 A 签发 Permit，状态为 `PermitIssued`；
3. 修订上游 Decision，使 Action A 被递归加入 `invalidated_refs`；
4. 调用 `mark_execution_permit_delivered()`；
5. 调用 `mark_execution_dispatched()`。

当前 transition 路径仍可成功。

**影响**

系统可以执行已失效的旧 Action。更危险的是，外部副作用可能已发生，而随后 Receipt 因 Action stale 被 Store 拒绝，形成“真实世界已改变、权威账本没有合法 Receipt”的不一致。

**推荐修复**

- Contract invalidation 必须同步取消未 dispatch 的 Reservation；
- 每次 Delivered/Dispatched 前重新检查 Action currentness、dependency snapshot 和 policy revision；
- 已 Dispatched 后发生 invalidation 时进入 `ReconciliationRequired`，不能继续普通成功路径；
- 增加 `ActionInvalidated` / `ReservationCancelled` 状态和攻击回归测试。

---

## P0-08：丢失 Permit 在 lease 到期后首次恢复会造成永久 Reservation 死锁

**代码证据**

- `idr-store/src/lib.rs:143-170`：只有 Failed、Rejected、Expired 允许 retry；
- `idr-store/src/lib.rs:588-633`：`recover_execution_permit()` 在时间过期时只返回 `ExecutionLeaseExpired`，没有把 attempt 状态持久化为 `Expired`；
- `idr-store/src/lib.rs:512-524`：创建下一 attempt 时，旧 attempt 必须处于 `permits_retry()` 状态。

**可复现方式**

1. Store 持久化 `PermitIssued`；
2. 进程在 Permit 返回调用者前崩溃，Permit 丢失；
3. 系统恢复时 lease 已过期；
4. 调用 `recover_execution_permit()`，收到 `ExecutionLeaseExpired`；
5. 尝试签发 attempt 2。

因为 attempt 1 在 snapshot 中仍是 `PermitIssued` 而不是 `Expired`，attempt 2 会被拒绝。没有可调用路径再改变它，因为调用者并没有丢失的 Permit 对象，无法使用 transition API。

**影响**

某个 Action/idempotency key 可被永久锁死，需要手工修改 Store 或放弃业务操作。该问题违背原始设计的可恢复性要求。

**推荐修复**

`recover_execution_permit()` 在持锁发现过期时必须原子持久化 `Expired`，或者提供不依赖 Permit 对象的 `expire_or_abandon_reservation(reservation_id, owner, proof)`。同时实现 owner lease renewal、takeover 和 bounded recovery。

---

## P0-09：Receipt Proof 的 authority context 复核不完整，且 commit 存在锁前时间 TOCTOU

**代码证据**

- `execution.rs:568-581`：Receipt 可以生成包含 tenant/scope/purpose/policy 的 expected binding；
- `execution.rs:602-622`：`verify_execution_receipt()` 只比较 proof kind、issuer、subject ref/digest、nonce 和时间，不比较 tenant、scope、purpose、policy revision；
- `idr-store/src/lib.rs:278-289`：Store 在取得文件锁前读取 clock 并调用 `validate_at()`；
- `idr-store/src/lib.rs:305-311`：真正的 Store file lock 在之后才取得；
- `execution.rs:637-647`：`validate_at()` 只验证时间，不复核 Trust Root 或完整 claims binding。

**可复现方式 A：错误 context 的 Verified Proof**

用公共 `ExpectedProofBindingV1::new()` 以错误 tenant/scope/purpose 验证一个 subject/digest/nonce 正确的 Provider Proof，再传入 `verify_execution_receipt()`。该函数不会发现 context 不匹配。

**可复现方式 B：过期竞态**

1. Receipt Proof 在 `commit_execution_receipt()` 开始时尚有效；
2. 线程等待 Store file lock；
3. Proof 在等待期间过期；
4. 取得锁后不再读取 trusted time，继续 commit。

**影响**

Receipt 可能被错误租户/范围的 Proof 接受，或在过期后持久化。Provider attestation 仍未成为锁内原子消费证明。

**推荐修复**

- `verify_execution_receipt()` 必须比较完整 expected binding；
- Store 在持锁并重载 snapshot 后重新读取 clock、Trust Root 和 claims；
- 保存/消费 proof ID，防止 Receipt Proof 重放；
- 把 Provider Proof verification 直接放进 Store/Orchestrator transaction，而不是接受永久 Verified wrapper。

---

## P0-10：Response Admission 仍可由调用者自报、重放和伪造发送时刻

**代码证据**

- `response.rs:188-257`：实际内容摘要绑定已经正确；
- `response.rs:272-286`：Policy facts 仍是普通 refs、时间和 bool；
- `response.rs:304-373`：Admission 是 pure function，并接受调用者传入的 `valid_at`。

没有 signed Policy Evaluation Proof、Store/Send façade 的可信时钟、single-use send permit 或 Delivery Receipt。

**可复现方式**

构造一个内容摘要匹配、所有 bool=true、时间窗口自洽的 facts，再选择落在窗口内的 `valid_at`，即可得到 Allow。相同 Allow 可被重复用于多次发送。

**影响**

系统能证明“内容没有被替换”，但不能证明“Policy Engine 确实批准”“当前仍有效”“只发送了一次”“实际发出的就是该内容”。

**推荐修复**

建立 `VerifiedResponseAdmissionProofV1 → single-use ResponseSendPermitV1 → DeliveryReceiptV1`，在发送边界用可信 clock 和 durable nonce 原子消费。

---

## P0-11：Human Model Gate 仍可绕过，且 Gate Decision 没有绑定具体 Assertion 内容

**代码证据**

- `human_model.rs:147-174`：`HumanModelAssertionV1` 仍可 `Deserialize`；
- `human_model.rs:176-235`：`issue()` 虽接收 Gate Decision，但 Assertion 不保存 Gate ref、Candidate ref、用户确认事件或 Outcome proof；
- `human_model.rs:343-353`：Gate facts 是普通 bool/计数；
- `human_model.rs:366-409`：Gate Decision 只保存 outcome/reasons/rule version；
- `human_model.rs:411-466`：evaluator 不绑定 subject、predicate、value、scope 或 evidence；
- `idr-store/src/lib.rs:1319` 附近：Human Model 的 Store 跨对象一致性分支为空。

**可复现方式**

- 从 JSON 反序列化 `epistemic_status=user_confirmed` 或 `outcome_supported` 的 Assertion；
- 或获取一次 `PromoteUserConfirmed` Gate Decision，用它签发多个内容、主体或范围不同的 Assertion；
- 或直接在 facts 中声明 `explicit_user_confirmation=true`。

**影响**

长期 Human Model 仍可被伪造或错误复用。`effective_value()` 只解决生命周期过滤，不解决 Assertion 如何获得高 epistemic status。

**推荐修复**

分离并持久化：

```text
HumanModelUpdateCandidateV1
HumanModelPromotionProofV1
HumanModelAssertionV1
```

Promotion Proof 必须绑定 candidate digest、subject、predicate、value、scope、evidence，以及具体用户输入或 Outcome Observation Proof；Store 只接受这条链生成的 sealed Assertion。

---

## P0-12：Outcome 仍没有真实 Observation Attestation

**代码证据**

- `outcome.rs:29-87`：Outcome 可公开反序列化和 issue；
- `outcome.rs:107-180`：主要验证结构、引用、状态和 attribution；
- Store 能检查 Action/Receipt 对应关系，但不能证明 `observed_results` 来自真实观察。

**可复现方式**

在已有 Action 和合法 Receipt 后，创建结构合法但业务结果虚假的 `Observed` Outcome，使用任意格式合法 evidence ref。

**影响**

虚假 Outcome 可以进入审计、成功评估和 Human Model 晋级链，造成错误归因与长期模型污染。

**推荐修复**

要求 Provider/Observer 签名的 `OutcomeObservationAttestationV1`，绑定 Receipt、观察窗口、observer、数据摘要、success criteria 和 attribution review。

---

## P0-13：Orchestration Runtime 仍不是可执行控制平面，也没有完整 renew/takeover/reconcile

**代码证据**

- `idr-orchestrator/src/lib.rs:1-5` 明确声明 V1.3 尚无 production repository 或 scheduler；
- `orchestrator.rs:229-236`：Run Record 只是数据结构；
- `orchestrator.rs:248-265`：只有 `load_run` 和 `compare_and_append` trait；
- 没有 repository 实现、event loop、scheduler、command handler、worker lease、reservation renewal、takeover 或 provider reconciliation loop。

**可复现方式**

触发以下任一场景：

- Permit Delivered 后 worker 崩溃；
- Permit Dispatched 后 Provider 结果未知；
- reservation owner 长时间失联；
- run cancellation 与 external dispatch 竞争；
- Action invalidation 与 execution transition 竞争。

当前没有一个运行中的 Orchestrator 负责统一裁决这些状态。

**影响**

原始设计要求的中断、恢复、等待、取消、重试、未知结果处理和失效协调尚未实现。各 crate 有正确的局部类型，但没有唯一控制者。

**推荐修复**

实现 durable Orchestrator aggregate、event log、scheduler、worker lease、renew/takeover、command processing、provider reconciliation 和 crash recovery；所有 Permit/Receipt/Outcome 必须由 Orchestrator step transition 触发。

---

## P0-14：Store anti-rollback 仍只依赖同一主机上的 snapshot 与 anchor

**代码证据**

- `idr-store/src/lib.rs:1-7` 明确说明不是 externally anchored immutable ledger；
- `idr-store/src/lib.rs:1452-1482` 比较本地 snapshot 与本地 anchor；
- `idr-store/src/lib.rs:1485-1577` 维护本地 append-only anchor 文件。

如果攻击者同时回滚 snapshot 和 anchor，二者仍然自洽。

**可复现方式**

备份某一旧 sequence 的 snapshot 与 anchor；系统运行产生新 Contract/Reservation/Receipt 后，同时恢复这两个旧文件。加载时本地链可重新自洽。

**影响**

拥有本地文件系统控制权的攻击者可以恢复旧 current pointer、撤销记录、Reservation 和 replay index。局部损坏检测较强，但强对手 anti-rollback 不成立。

**推荐修复**

将每个 snapshot root/sequence 提交到独立 WORM、透明日志、远端 KMS monotonic counter 或独立审计服务。生产加载必须验证外部 checkpoint。

---

# 九、P1 问题

## P1-01：Trust Root 配置和可信时间仍缺少受保护生产实现

**证据**：`trust.rs:389-407` 的 Trust Root 由调用者提供 key policy；`trust.rs:410-420` 使用系统墙钟；Store 的 `SystemTrustedClockV1` 位于 `idr-store/src/lib.rs:173-188`。  
**复现**：错误配置 key policy、系统时钟回拨或管理员替换 Trust Root，当前没有硬件/远端 monotonic source 保护。  
**修复**：使用受保护配置、key epoch、远端 revocation distribution、monotonic/hybrid trusted clock，并审计配置变更。

## P1-02：Store 重放不会使用当前 Trust Root 对历史 Proof 重新验签

**证据**：`idr-store/src/lib.rs:880-1057` 重放验证 proof envelope 形状、字段对应和状态，但 Store 不持有 `ProofTrustRootV1`；历史 Provider Proof 同样只做结构复核。  
**复现**：历史 key 后续被认定泄露或撤销，重新加载 snapshot 时无法按当前 policy 标记相关 Permit/Receipt 为需复核。  
**修复**：Ledger replay 支持 trust epoch、historical key policy、revocation checkpoint 和 cryptographic re-verification；不能只相信 snapshot 中的 envelope。

## P1-03：Decision selected option 与 Action operation/parameters 没有语义编译证明

**证据**：Store 可以检查 Decision authority boundary 和 Action 引用，但没有 `DecisionEffectBindingV1` 证明所执行 Action 是 selected option 的合法编译结果。  
**复现**：Decision 选择 option A，但关联 Action 使用与 A 无关的 operation/parameters，只要结构和 authority boundary 合法即可。  
**修复**：Decision 输出规范化 effect plan；Action 必须引用签名 compiler proof，绑定 selected option、operation、参数摘要和 constraints。

## P1-04：Lineage 永久固定同一 run/turn，可能阻止合法跨 Turn 修订

**证据**：`idr-store/src/lib.rs:1137-1144` successor continuity 要求 run ID、turn ID 和 Authority Context 全部与 predecessor 相同。  
**复现**：用户在新 Turn 修正一个长期 Intent/Decision/Human Model lineage，合法 successor 会因 turn 不同被拒绝。  
**修复**：区分 immutable lineage identity 与 transition context；跨 Turn 修订必须携带 typed transition proof，而不是永久禁止。

## P1-05：Contract、Action parameters 和结果摘要仍没有语言中立 canonical encoding

**证据**：`common.rs:379-386` 附近的 `contract_digest()` 仍基于 Rust `serde_json::to_vec`；Proof claims 有自定义 canonical bytes，但普通 Contract 没有。  
**复现**：跨语言对象键顺序、数字表示、Unicode 归一化或 map 实现差异可产生不同 digest。  
**修复**：冻结 IDR Canonical Encoding V1（如严格 JCS + IDR 类型约束），发布 Rust/TS/Python byte-for-byte golden vectors。

## P1-06：Store 仍是 whole-snapshot backend，没有 WAL、transactional outbox 和多节点 fencing

**证据**：`idr-store/src/lib.rs:1400-1577` 采用完整 snapshot rewrite、rename、fsync 和本地 anchor。  
**复现**：高频 Contract/Reservation 下文件增长导致写放大；外部系统 dispatch 与 snapshot commit 之间没有 transactional outbox；多节点依赖共享文件锁而非共识/fencing token。  
**修复**：append-only WAL、checkpoint、outbox、crash injection、多节点 leader fencing/consensus，以及可证明的 dispatch handoff。

## P1-07：Human Model `effective_value()` 不是强制消费边界，Usage Policy 未在读取时执行

**证据**：`human_model.rs:323-336` 提供正确过滤 API，但 Assertion 仍暴露结构和值；没有 Human Model repository/query façade 强制所有消费者使用 `effective_value()` 并检查 allowed uses、domain/task scope 和 maximum impact。  
**复现**：Host 直接反序列化 Assertion 并读取原始 `value`，绕过生命周期和 usage policy。  
**修复**：隐藏原始值访问，提供受治理 query API，绑定 consumer purpose/impact，并记录每次模型使用。

## P1-08：Response、Outcome 和 Human Model 状态语义仍不够强

**证据**：结构校验没有完整规定 PartiallyObserved/Conflicted 的证据基数、Outcome attribution 审查、Human Model source type 与 epistemic status 的允许矩阵。  
**复现**：构造形式合法但证据语义薄弱的状态，例如高置信 OutcomeSupported 但只有一个不透明 evidence ref。  
**修复**：建立 per-state normative invariants、evidence cardinality、confidence ceilings 和 provenance requirements。

## P1-09：跨语言协议只覆盖边界子集，仍由多份手写实现维护

**证据**：TS/Python 对 Canonical Input 和 assessment 的基本字段已对齐，但完整 Contract、Proof、Reservation、Receipt、Outcome 和 Human Model 没有单一 schema 生成及跨语言 golden signature vectors。  
**复现**：Rust 新增字段后，TS/Python fixture 未覆盖时，漂移只能依赖人工发现。  
**修复**：从规范 schema 生成三语言类型/validator，CI 执行 full-contract round-trip、canonical bytes 和签名向量比较。

## P1-10：关键攻击、崩溃和并发路径仍缺少动态测试

**证据**：Round 5 新增测试覆盖双 Authorization、并发 Store、正常恢复、Receipt mismatch、failed→attempt2 等，但未覆盖：

- Permit issuance 后 Action invalidation；
- lost Permit 第一次恢复时已过期；
- key 在 verify 后、consume 前撤销；
- Receipt Proof 错误 tenant/scope/purpose；
- Receipt 等锁期间过期；
- Human Model wire promotion/Gate reuse；
- snapshot+anchor 同时回滚；
- Orchestrator crash/restart/takeover/reconciliation。

**复现**：将上述静态路径写成多进程、可控 clock、crash injection 测试。  
**修复**：把每个 P0 的复现步骤转成 Rust integration/property/fuzz/fault-injection 测试，并作为发布必过门槛。

---

# 十、P2 问题

## P2-01：协议版本协商和迁移机制不足

**证据**：schema/rule version 主要为固定常量，没有 minimum reader、migration contract、deprecation window 或 mixed-version replay policy。  
**复现**：旧节点读取新增字段或新节点重放旧 snapshot 时，只能整体拒绝或依赖临时代码。  
**修复**：定义版本兼容矩阵、migration events、minimum reader/writer 和升级回滚规则。

## P2-02：资源预算尚未覆盖整个运行和审计平面

**证据**：部分文本、JSON payload 已有限制，但 Proof/Orchestration events、Reservation attempts、anchor records、全 run event 数和全局 evidence refs 缺少统一预算。  
**复现**：持续创建大量合法 attempts/events/refs，造成 snapshot 膨胀和验证 CPU/内存压力。  
**修复**：建立 per-contract、per-run、per-tenant 和 per-snapshot quotas，以及压缩/归档策略。

## P2-03：Orchestration Run Record 信息不足

**证据**：`orchestrator.rs:229-236` 只有 run ID、state、cancel flag、last sequence。  
**复现**：生产恢复时无法仅凭 Run Record 得知 tenant、subject、root intent、current step、wait reason、policy revision、owner/lease、failure cause 和 terminal outputs。  
**修复**：扩充 durable aggregate state，或明确所有信息必须可由 event log 确定重建并提供 projection。

## P2-04：部分 Turn Coordination 模式在当前 assessment 中缺乏清晰可达语义

**证据**：协议包含 `ActThenRespond` 等模式，但当前 runtime/request 组合和 Turn Contract 约束使部分模式接近保留状态。  
**复现**：构造 action-only 或 response-optional 场景，检查 selector 与 Turn Plan 是否都有合法表示。  
**修复**：为每种 mode 定义规范用例和可达性测试；删除不支持模式或补齐 Contract 表达。

## P2-05：发布清单本身没有外部签名

**证据**：`SHA256SUMS` 能验证包内一致性，但没有发布者签名、SLSA provenance 或透明日志记录。  
**复现**：攻击者同时替换 ZIP 和其中的 `SHA256SUMS`，内部校验仍可全部通过。  
**修复**：使用独立发布 key 签名清单，并提交透明日志/构建 provenance。

## P2-06：审计包包含 Python `__pycache__`，与 Validation Summary 的排除声明不一致

**证据**：包中存在 4 个 `__pycache__/*.pyc`；`VALIDATION-SUMMARY.md` 却声明 Python caches 已排除。  
**复现**：检查：

```text
IDR/research/evaluation/src/idr_eval/__pycache__/*
IDR/research/evaluation/tests/__pycache__/*
```

**影响**：不构成核心安全漏洞，但降低打包声明的准确性，并引入解释器版本相关二进制噪音。  
**修复**：发布脚本强制清理缓存，并让 manifest generation 对禁入路径失败。

---

# 十一、关键可复现攻击路径

## 11.1 Permit 签发后 Action 失效仍然 dispatch

```text
Commit Action A
→ issue Permit A
→ supersede upstream Decision
→ recursive invalidation marks Action A stale
→ mark Permit Delivered
→ mark Permit Dispatched
→ executor performs stale effect
```

预期：在 Delivered/Dispatched 前拒绝并取消 Reservation。  
当前：transition 不查看 invalidated refs。

## 11.2 Lost Permit 过期恢复造成永久锁死

```text
Persist PermitIssued
→ crash before permit reaches owner
→ wait until lease expires
→ recover_execution_permit() returns expired error
→ state remains PermitIssued
→ attempt 2 rejected forever
```

预期：恢复路径原子标记 Expired，并允许连续 attempt。

## 11.3 Revoked key 的已验证 Proof 延迟消费

```text
verify Proof at t1
→ key revoked at t2
→ keep VerifiedProof in memory
→ Store consumes at t3
```

预期：Store 使用当前 Trust Root 再验。  
当前：Store 不持有 Trust Root，只调用 wrapper 的 `validate_at()`。

## 11.4 不经过 Action Admission 直接签发 Permit

```text
Action + ordinary Approve Authorization
→ ExecutionPermitRequest
→ trusted permit issuer signature
→ Store issue_execution_permit
```

预期：必须绑定完整 ActionAdmissionProof。  
当前：Store 没有该依赖。

## 11.5 Receipt 锁前有效、锁后过期

```text
clock read + validate_at
→ wait on Store file lock
→ proof expires
→ acquire lock
→ commit without second time check
```

预期：锁内重读可信时间并重验。

## 11.6 Human Model 直接晋级

```text
Deserialize Assertion with user_confirmed
or
reuse one gate decision for unrelated assertions
```

预期：Store 要求 content-bound Promotion Proof。  
当前：Assertion 本身不保存证明链。

---

# 十二、推荐修复方案与下一阶段顺序

## 阶段 1：先修复 Reservation 的两个新 P0

1. invalidation 时原子取消所有未 Dispatched reservation；
2. Delivered/Dispatched 前重验 Action currentness 和 dependency snapshot；
3. `recover_execution_permit()` 在过期时持久化 `Expired`；
4. 增加 owner lease、renew、abandon、takeover；
5. 写入两条必过攻击测试：stale Permit dispatch、expired lost Permit retry。

这是 Round 6 的最高优先级。

## 阶段 2：让 Action Admission 成为 Permit 的唯一证明来源

1. 把 capability、actor/agent authority、policy、precondition、verification、compensation、dependency currentness 变为 typed proofs；
2. 生成 content-bound `ActionAdmissionProofV1`；
3. Permit Request 必须引用并签名绑定该 proof；
4. Store 在 issuance 中验证并消费 admission proof。

## 阶段 3：把 Trust Root 验证移动到消费事务

1. Store/Orchestrator 持有 current Trust Root handle；
2. 锁内重验 Permit/Receipt signed envelope；
3. 检查 key epoch、revocation generation、policy revision；
4. Receipt commit 在锁内重读 trusted clock；
5. 完整比较 tenant/scope/purpose/policy binding。

## 阶段 4：封闭 authoritative Contract 和 Authorization

1. authoritative constructors 改为 crate-private；
2. wire Candidate/Submission 与 stored authoritative object 分离；
3. Runtime 签发 sealed envelope；
4. Store 只接受 Runtime seal；
5. User Authorization 由 authenticated confirmation + authority proof 生成并签名。

## 阶段 5：闭合 Response、Outcome、Human Model

```text
VerifiedResponseAdmission
→ single-use Send Permit
→ Delivery Receipt

Provider Receipt
→ Outcome Observation Attestation
→ Outcome Record

Human Model Candidate
→ Promotion Proof
→ sealed Assertion
```

## 阶段 6：实现真正 durable Orchestrator

实现 repository、scheduler、worker lease、CAS state transition、cancel/invalidate、retry、takeover、provider reconciliation 和 crash recovery。所有 Contract/Permit/Receipt/Outcome 必须由 Orchestrator step 推进。

## 阶段 7：生产 Durability 与跨语言标准

1. append-only WAL + checkpoint；
2. transactional outbox；
3. external WORM/transparency checkpoint；
4. multi-node fencing；
5. canonical encoding；
6. Rust/TS/Python byte/signature golden vectors；
7. power-loss、crash、concurrency、fuzz 和 rollback 测试。

## 阶段 8：Aegis 生产适配

只有在前述链路关闭后，Aegis 才能从 shadow adapter 升级。Adapter 必须只提交 authenticated candidate/input，并消费 IDR sealed outputs；权限只能收窄，不能扩大。

---

# 十三、解除 BLOCKED 的最低门槛

```text
Authoritative contracts sealed issuance = PASS
Production feature gates technically enforced = PASS
Authenticated ingress + nonce replay prevention = PASS
Intent/Decision/Turn facts proof-only = PASS
Signed user authorization + online revocation = PASS
Action Admission bound to Permit = PASS
Invalidated Action cancels/prevents Permit dispatch = PASS
Expired/lost Permit recovery = PASS
Trust Root recheck at consumption = PASS
Provider Receipt full context binding = PASS
Receipt lock-time freshness = PASS
Response send permit + delivery receipt = PASS
Outcome observation attestation = PASS
Human model promotion proof chain = PASS
Runnable durable orchestrator = PASS
External anti-rollback checkpoint = PASS
Cross-language canonical vectors = PASS
Critical crash/concurrency tests = PASS
P0 = 0
```

---

# 十四、最终判断

Round 5 已经正确解决了 Round 4 最突出的“双执行资格通道”和“同 Action 多 attempt-1 Permit”问题。它还把 Receipt 与 Permit、provider、owner、attempt 和 nonce 的结构关系大幅加强。这些改动值得保留。

但是，生产 Trust Root 的标准不是“正常路径可以运行”，而是：

```text
每一个影响真实世界的动作
必须始终基于当前有效 Action
必须经过不可伪造的 Admission 与 Authorization
必须只有一个可恢复执行资格
必须在崩溃、撤销、失效和结果未知时仍保持一致
必须能证明 Receipt、Outcome 和 Human Model 来自真实证据
```

Round 5 仍存在：

```text
Action 失效后旧 Permit 可 dispatch
lost Permit 过期恢复会永久锁死
Trust Root 撤销不在消费时复核
Action Admission 不约束 Permit issuance
Authorization/Input/Response/Human Model 仍有自声明入口
Outcome 无真实观察证明
Orchestrator 尚未运行
本地 Store 无外部 anti-rollback
```

因此，**Round 5 仍不能被认定为 The Human-Centered Intent & Decision Runtime V1.3 的生产信任根。**

它当前最准确的定位是：

> **一个质量明显提升、具有单节点 Execution Reservation 和 Provider-signed Receipt 基础的 IDR Protocol/Foundation 实现；可以继续用于 shadow mode、攻击测试和下一轮闭环开发，但不得进入生产授权、生产执行或长期 Human Model 写入。**
