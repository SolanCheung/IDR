# IDR V1.3 Trust-Chain Closure Round 7 独立复审报告

审计日期：2026-07-30  
审计对象：`IDR-V1.3-trust-chain-closure-round7-2026-07-29.zip`  
ZIP SHA-256：`94ff77bc0ab367e64cf251b9fe969089d04a71bdc76335166c4cbd47ed5c7abb`

## 1. 总体结论

Round 7 是一次有效的结构性整改。上一轮发现的公共 Repository authority laundering、默认生产 API 暴露 `PgPool`、Decision→Action 派生缺失、successor 不失效下游、终态 Run 仍可继续 Reserve/Dispatch、owner/provider 不校验、错误 operation idempotency、Receipt request digest 不绑定以及 command ID 冲突等问题，在默认 Round 7 路径中已经得到实质修复。

但是 Round 7 **没有通过第一次独立干净复审**。本轮仍确认 5 个信任边界级 P0，其中两个属于上一轮问题没有完全关闭，三个属于 Round 7 代码与 Master Spec 之间的新发现。

```text
ROUND7_FIRST_CLEAN_INDEPENDENT_REVIEW = NO
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

严重度汇总：

```text
P0 = 5
P1 = 12
P2 = 4
```

本轮不能接受 `ROUND6_P0_14_ALL_CLOSED`。更准确的判断是：

```text
Round 6 P0 fully closed = 10/14
Round 6 P0 partially closed = 4/14
New trust-boundary P0 = 3
```

其中部分问题归并到相同根因后，最终列为 5 项 P0。

---

## 2. 独立验证范围

### 2.1 包完整性

独立检查结果：

- ZIP SHA-256 与提交值一致；
- ZIP 条目：993；
- 有效普通文件：818；
- `SHA256SUMS` 条目：817；
- `SHA256SUMS`：817/817 PASS；
- 路径穿越：0；
- 符号链接：0。

额外的第 818 个普通文件是 `SHA256SUMS` 本身。

### 2.2 独立执行结果

当前审计容器没有 `cargo`、`rustc` 和 `psql`，因此不能独立执行 Rust/PostgreSQL 测试。本报告没有把包内维护者运行日志等同于独立执行证据。

独立执行：

- TypeScript `npm ci --offline`：PASS；
- TypeScript tests：12/12 PASS；
- TypeScript typecheck：PASS；
- Python tests：9/9 PASS。

包内日志声称以下结果通过，且日志/文件摘要完整，但属于维护者环境证据：

- IDR Rust：59 项；
- compile-fail：7 项；
- strict Clippy；
- rustfmt；
- PostgreSQL；
- Aegis Life：1,878 项。

---

## 3. Round 6 十四项 P0 关闭矩阵

| Round 6 问题 | Round 7 结论 | 说明 |
|---|---|---|
| P0-01 Repository authority laundering | **关闭** | 公共 Repository trait、泛型 authority orchestrator 和公共 transition 已删除/降为 crate-private |
| P0-02 writable `PgPool` | **部分关闭** | `production` 单 feature 下关闭，但 `production + test-support` 会重新暴露 |
| P0-03 raw projection tampering | **部分关闭** | 正常路径有 projection digest/audit 复核；`test-support` 的 raw pool 可绕过 |
| P0-04 subject/turn lineage switching | **关闭** | Runtime 中央校验及 PostgreSQL trigger 双重执行 |
| P0-05 Decision→Action selected-option derivation | **关闭** | option、operation、parameter digest 精确比较及 DB trigger |
| P0-06 successor downstream invalidation | **关闭** | Store 事务中计算 reverse dependency closure 并更新 projection |
| P0-07 terminal Run gate | **关闭** | command-state gate 已覆盖 Reserve/Deliver/Dispatch 等路径 |
| P0-08 execution lifecycle caller authority | **关闭** | owner/provider caller binding 已实现；但 Proof exact target 另有新 P0 |
| P0-09 operation-level idempotency | **关闭** | Reservation 使用真实 operation ref |
| P0-10 Receipt request digest | **关闭** | exact Action/operation/parameter derivation形成 request digest 并复核 |
| P0-11 Human Model content substitution | **部分关闭** | predicate/value/scope/evidence 已内部物化；lifecycle/impact 仍可升级 |
| P0-12 exact Outcome binding | **关闭** | Candidate/Promotion 绑定 exact Outcome ID/revision/digest |
| P0-13 command ID conflicting replay | **关闭** | tenant/run/actor/caller/full command digest 冲突拒绝 |
| P0-14 unauthenticated inspect/HM/outbox controls | **部分关闭** | 默认 production 隐藏；`production + test-support` 再次公开 |

---

# 4. P0 问题

## P0-01：`production + test-support` 重新打开原始数据库和未认证控制面

### 证据

`crates/idr-runtime/Cargo.toml:7-15`：

```toml
production = ["postgres-authority", "idr-protocol/production"]
test-support = []
```

两者是相互独立、可叠加的公共 Cargo feature，没有：

```rust
compile_error!(all(feature = "production", feature = "test-support"));
```

`crates/idr-runtime/src/postgres_authority.rs:302-305`：

```rust
#[cfg(feature = "test-support")]
pub fn pool_for_test(&self) -> &PgPool
```

同一 feature 还公开：

- `query_effective_human_model_assertions_for_test`：311-386；
- `claim_outbox_events_for_test`：388-443；
- `acknowledge_outbox_event_for_test`：445 起；
- `inspect_for_test`：1015-1044；
- `verify_external_anchor_for_test`：1046-1049。

`tools/production-api-compile-fail/Cargo.toml:9-10` 只验证：

```toml
features = ["production"]
```

没有验证 `features = ["production", "test-support"]`。

文档 `docs/trust-chain-closure/API-VISIBILITY.md` 声称这些控制“Not compiled into production feature”，但 Cargo features 是 additive；该说法对 feature-unification 场景不成立。

### 可复现方式

创建一个下游 crate：

```toml
[dependencies]
idr-runtime = {
  path = ".../crates/idr-runtime",
  default-features = false,
  features = ["production", "test-support"]
}
```

然后：

```rust
let pool = orchestrator.pool_for_test();
sqlx::query("UPDATE idr_runs SET projection = '{}'::jsonb")
    .execute(pool)
    .await?;
```

预期：当前 feature 图没有编译阻断，方法会被公开。

### 影响

普通 Rust 调用者重新获得：

- 权威表直接写入；
- projection/audit/outbox/Human Model 绕过；
- 跨租户查询与 outbox claim；
- Proof、CAS、Orchestrator 和审计链全部旁路。

### 推荐修复

首选：

```rust
#[cfg(all(feature = "production", feature = "test-support"))]
compile_error!("production and test-support are mutually exclusive");
```

更稳妥：完全删除生产 crate 的 `test-support` public feature，把测试辅助接口移到：

- 单独的 `idr-runtime-testkit` dev-only crate；或
- crate 内 `#[cfg(test)]` 模块；或
- PostgreSQL integration tests 自己持有测试 pool，不从 production Orchestrator 暴露。

新增 Cargo feature-unification compile-fail 测试。

---

## P0-02：生产发布门禁不是独立强制门禁，Human Model gate 实际为 no-op

### 证据

`crates/idr-protocol/src/production.rs:23-40` 定义六个门禁：

- trust-chain closure；
- production trust root；
- production authorization；
- production execution；
- Human Model long-term write；
- Aegis production adapter。

但全仓库搜索显示：

- `production_authorization_is_enabled()` 没有生产调用；
- `production_execution_is_enabled()` 没有生产调用；
- `human_model_long_term_write_is_enabled()` 没有生产调用。

`crates/idr-runtime/src/postgres_authority.rs:259-271` 的 Production 启动只检查：

```rust
ProductionReleaseGatesV1::BLOCKED.production_trust_root_is_blocked()
anchor_backend.is_production_durable()
```

`crates/idr-runtime/Cargo.toml:14-15` 虽定义：

```toml
human-model-write = []
```

但该 feature 在源码中没有任何 `#[cfg(feature = "human-model-write")]` 使用；`PromoteHumanModelAssertion` 和 correction 在所有 PostgreSQL authority 构建中均可执行。

### 可复现方式

代码层面将 `production_trust_root_blocked` 改为 false，但保持：

```text
production_authorization_enabled = false
production_execution_enabled = false
human_model_long_term_write_enabled = false
```

Production startup 将不再被第一项阻断，而 `AdmitAction`、`ReserveExecution`、`StartDispatch`、`PromoteHumanModelAssertion` 没有任何独立 flag 检查。

当前常量全部 blocked，因此尚未形成现实发布；但门禁结构并不能支持逐项安全开启。

### 影响

一旦解除 Trust Root 总门禁，系统会同时开启授权、执行和长期 Human Model 写入，违背独立发布门禁设计。当前 `HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED` 只是声明，不是代码强制。

### 推荐修复

在 Production `handle()` 的最前面增加 command-class gate：

```text
Authorization commands → require production_authorization_enabled
Execution commands → require production_execution_enabled
Promote/Correct HM → require human_model_long_term_write_enabled
```

并让 `human-model-write` feature 真正包围：

- command variants 或 command dispatch；
- PostgreSQL specialized tables/write code；
- query API；
- migrations/compatibility gate。

增加逐门禁测试，禁止仅检查总 Trust Root gate。

---

## P0-03：`Shadow` 是调用者可选的发布门禁旁路，且其权威记录没有 trust-domain 标记

### 证据

`crates/idr-runtime/src/postgres_authority.rs:159-163`：

```rust
pub enum PostgresStartupModeV1 {
    Shadow,
    Production,
}
```

`connect()` 在 `251-271` 仅当调用者选择 `Production` 才检查 release latch 和 durable anchor。选择 `Shadow` 可直接创建同一个 `PostgresIdrOrchestratorV1`。

`handle()`：`992-1013` 对 Shadow 与 Production 没有命令能力差异，均运行完整：

- Action Admission；
- Execution Reservation/Permit/Dispatch；
- Receipt/Outcome；
- Human Model Promotion/Assertion。

`AuthoritativeRecordV1`：`trust_chain.rs:104-119` 没有：

```text
trust_domain
startup_mode
shadow_marker
environment_id
```

数据库 `0001_idr_v13_trust_chain.sql:5-70` 的 `idr_runs`、`idr_contract_records` 也没有上述隔离字段。

此外，`postgres-authority` 可以不启用 `production` 独立编译；`idr-store` 默认还包含 `postgres-store`。

### 可复现方式

```rust
PostgresIdrOrchestratorV1::connect(
    production_database_url,
    production_schema,
    PostgresStartupModeV1::Shadow,
    caller_controlled_security_context,
).await?;
```

随后调用同一个 `handle()` 写入 Action、Execution、Outcome 或 Human Model。

这些记录与未来 Production 模式生成的记录在类型和表结构上不可区分。

### 影响

- release latch 取决于调用者选择的 enum，而不是部署隔离；
- Shadow artifact 可污染生产 schema；
- 后续 Production 进程无法仅凭记录判断其来源；
- 违反 Master Spec 的 shadow/production feature separation 和 fail-closed 要求。

### 推荐修复

两种可接受方案：

1. **编译隔离**：独立 `shadow-mode` feature，和 `production` 互斥；Shadow 使用不同类型和不同 store schema。
2. **强 trust-domain 隔离**：所有 Run、Record、Proof consumption、Audit、Outbox 均绑定 immutable `trust_domain/environment_id`；Production 启动拒绝读取任何 Shadow lineage。

不要以公开 runtime enum 作为唯一安全边界。

---

## P0-04：Proof 没有绑定完整 command envelope，可重定向 Run 或 exact target

### 根因证据

`IdrCommandEnvelopeV1` 包含：

- `run_id`；
- `expected_aggregate_version`；
- actor/caller/tenant/scope/purpose/policy；
- command；
- proofs。

见 `trust_chain.rs:259-275`。

但 `proof_subject()`：`418-429`：

```rust
if let Some(candidate) = self.command.candidate() {
    return candidate.canonical_digest();
}
let digest = canonical_digest_v1("idr-command-subject-v1", &self.command)?;
```

也就是说：

- Candidate command 的 Proof 不绑定 command 中除 Candidate 外的 exact target refs；
- 非 Candidate command 的 Proof 只绑定 `IdrCommandV1`，不绑定 envelope `run_id`、expected version、correlation/causation。

### 攻击 A：跨 Run 重定向 `CancelRun`

`CancelRun` 的 command 内容只有 `reason_ref`，目标 Run 位于 envelope `run_id`。

步骤：

1. 为 Run A 构造并签名 `CancelRun { reason_ref }`；
2. 在 Proof 尚未消费前，序列化 envelope；
3. 仅修改 JSON 中 `run_id` 和 `expected_aggregate_version` 为 Run B；
4. 保留 command ID、command、actor/caller、tenant/scope/purpose/policy 和 Proof；
5. 重新反序列化并提交。

`proof_subject()` 仍得到相同 subject ref/digest；Proof 验证可通过。transition 加载的是 Run B projection，并可取消 Run B。

签发者从未批准 Run B。

### 攻击 B：Human Model correction 重定向到更新 revision

`CorrectHumanModelAssertion`：`trust_chain.rs:240-243` 同时包含：

```text
assertion_ref
correction Candidate
```

Proof 只绑定 correction Candidate digest，不绑定 `assertion_ref`。

用户针对 assertion v1 签署 correction 后，在提交前若 assertion 已变为 v2，调用者可以替换 command 中 `assertion_ref=v2`。Runtime 只要求它是 exact current ref，因此旧授权可修改或删除用户没有审阅的 v2。

`PromoteHumanModelAssertion` 的 `promotion_decision_ref` 存在同类问题。

### 影响

Proof 不再是 exact-object authorization，而是可在提交前重新选择目标。它破坏：

- exact authorization；
- stale revision protection；
- cross-run isolation；
- Human Model correction consent。

### 推荐修复

定义唯一的 proof subject digest：

```text
command_id
run_id
expected_aggregate_version
actor_ref
caller_ref
tenant_ref
scope_ref
purpose_ref
policy_revision_ref
command payload（含所有 exact refs）
```

排除的只能是 Proof envelopes 自身，不能排除 target refs 或 run ID。

可保留 Candidate digest作为内部字段，但所有生产 Proof 最终必须绑定完整 `UnsignedCommandEnvelopeV1` digest。

新增攻击测试：

- signed CancelRun A → mutate run to B；
- signed correction for assertion v1 → target v2；
- signed assertion promotion for decision p1 → target p2；
- mutate expected aggregate version；
- mutate correlation/causation（至少审计完整性拒绝）。

---

## P0-05：Human Model Assertion 可升级 lifecycle 和 impact，Promotion Decision 并未决定最终晋级语义

### 证据

`PromoteHumanModelAssertion`：`trust_chain.rs:1526-1585`：

1. 只检查 Promotion outcome 属于三个允许值之一；
2. 调用 `validate_human_model_assertion_request()`；
3. 从 assertion request 取得：
   - `lifecycle_state`；
   - `maximum_impact_basis_points`；
4. 覆盖到已物化 Candidate payload。

`validate_human_model_assertion_request()`：`2259-2291` 独立允许：

```text
provisional
user_confirmed
outcome_supported
```

它没有要求 lifecycle 与 Promotion outcome 对应，也没有要求 Assertion impact 不高于 Candidate proposed impact。

因此：

```text
Promotion outcome = promote_provisional
Assertion lifecycle = user_confirmed
```

会通过。

同样：

```text
Candidate maximum impact = 100
Assertion request maximum impact = 10000
```

会通过。

数据库 trigger `0002_round7_integrity_and_idempotency.sql:148-179` 只比较：

- predicate；
- value digest；
- scope；
- evidence；
- allowed purposes；
- Outcome digest。

它没有比较 lifecycle 或 maximum impact。

现有回归 `postgres_vertical_slice.rs:987-1039` 只测试替换 predicate，合法路径仍由请求传入 `provisional` 和 `100`，没有测试 lifecycle/impact escalation。

### 可复现方式

创建 Candidate：

```json
{
  "predicate": "prefers_explicit_confirmation",
  "maximum_impact_basis_points": 100,
  "...": "..."
}
```

创建 Promotion：

```json
{
  "outcome": "promote_provisional",
  "independent_evidence_count": 2
}
```

提交 Assertion request：

```json
{
  "source_candidate_digest": "<exact>",
  "lifecycle_state": "user_confirmed",
  "maximum_impact_basis_points": 10000
}
```

当前 Runtime 和 DB trigger 都没有拒绝条件。

### 影响

- 未经用户 Confirmation Proof 的 assertion 可表现为 `USER_CONFIRMED`；
- provisional evidence 可伪装成 outcome-supported；
- 低风险偏好可被扩大到高影响用途；
- 长期 Human Model trust status 和 usage boundary 被调用者提升。

### 推荐修复

Assertion request 不应携带这两个可升级字段。

Runtime 必须内部推导：

```text
promote_provisional       → provisional
promote_user_confirmed    → user_confirmed
promote_outcome_supported → outcome_supported
```

`maximum_impact_basis_points` 必须：

```text
assertion impact <= candidate proposed impact
assertion impact <= promotion/policy ceiling
```

最好完全从 Candidate + Policy Decision 内部计算。

DB trigger 同步强制：

- lifecycle 与 Promotion outcome 精确映射；
- assertion impact 不得大于 source Candidate impact；
- Promotion Decision exact ref 必须进入 assertion materialization digest。

---

# 5. P1 问题

## P1-01：缺少 WORM/KMS 外部 anchor 与受保护 checkpoint signer

项目已明确承认。当前 checkpoint 对象没有部署级签名和不可回滚外部持久化。

## P1-02：Trust Root/issuer registry 仍是进程内可注入对象

`CurrentTrustRootProviderV1` 和 `AuditCheckpointAnchorBackendV1` 是公共 trait；`RotatingTrustRootProviderV1` 是进程内对象。需要受保护控制服务、KMS/HSM、rotation approval 和分发审计。

## P1-03：Anchor 在 DB commit 后发布，存在 unanchored tail

`handle()` 在事务提交后调用 anchor backend。虽然重启可检测，但 commit 到 publish 之间仍有窗口。

## P1-04：内部 Permit 不是 provider-facing signed capability

Provider 尚不能只凭不可伪造、最小权限、短期签名 Permit 执行。

## P1-05：Response 没有 delivery/fan-out Receipt

当前只消费 send admission/nonce，不能证明哪个 channel、哪个 recipient 真正收到内容。

## P1-06：Capability/Authority/Policy registry 与 issuer separation 不完整

单一 expected issuer map 可由调用者配置，角色独立性、权限收窄和 registry revision 尚不充分。

## P1-07：Outcome conflict、attribution、observation aggregation 仍过于简化

Outcome 主要是单 Observation Proof + receipt digest，没有冲突观察、归因审查、时间窗和多证据聚合。

## P1-08：Human Model sensitive inference、retention、decay 和生产 Query API 不完整

有效 Human Model query 目前只有 `test-support` 方法；生产业务无法通过规范 Query API 安全消费 assertion。Retention/decay/sensitive policy 仍未完成。

## P1-09：Reconciliation 可以直接把 execution 标为 Succeeded，但不生成完整 Receipt

`trust_chain.rs:1369-1390`：`ReconcileExecution` 可将 state 设为 Succeeded/Failed/Rejected/Compensated，但不创建 Execution Receipt，也没有 result digest、provider execution ID 和完整时间字段。

这不应替代 Master Spec 要求的：reconciliation 成功后提交/重建 exact Receipt。

## P1-10：Run/Orchestration 状态机尚不完整

生产 transition 只显式把 Run 设置为：

- Running；
- Rejected；
- Invalidated；
- ReconciliationRequired；
- Cancelled。

正常 Receipt success/failure 不会把 Run 设置为 Succeeded/Failed。`persist_step_snapshot()` 又把大量 Running command 近似记录为 `completed`。这不是完整 durable orchestration semantics。

## P1-11：Migration/replay/fuzz/chaos 证据未达到发布阈值

本轮测试数量增加，但长期 fuzz、迁移跨版本 corpus、断电/crash matrix、长时多进程和 backup/restore 仍未完成生产阈值。

## P1-12：Aegis production adapter 和连续两轮独立复审仍未完成

当前 Aegis production adapter 正确 compile-blocked；本轮又发现 P0，因此不能计为第一轮 clean review。

---

# 6. P2 问题

## P2-01：`with_proofs` 注释与实际 command digest 不一致

`trust_chain.rs:350-363` 声称附加 proofs 不改变 command digest，但 `canonical_digest()` 对整个 envelope 序列化，proof 列表会改变 digest。应明确区分：

- unsigned command subject digest；
- full envelope storage/idempotency digest。

## P2-02：Round 7 文档的“14 个 P0 已处理”表述过度

`ROUND7-REAUDIT-RESPONSE.md` 对 P0-02、P0-03、P0-11、P0-14 的关闭说明只覆盖默认 feature/内容替换路径，没有覆盖 feature unification、lifecycle/impact 和 exact proof target。

## P2-03：默认 feature 同时包含 dev-file-store、Postgres 和 legacy-shadow API

`idr-store` 默认：

```toml
default = ["dev-file-store", "postgres-store"]
```

`idr-runtime` 默认又包含 `legacy-shadow-api`。虽然 production+dev store 有 compile gate，但默认组合增加误用和审计复杂度。建议 production-facing crates 默认最小化，dev/shadow 显式启用。

## P2-04：IDR 目录没有 Git 历史

SHA manifest 能固定交付快照，但无法验证 Phase commit、review provenance、bisect 和变更作者链。正式进入持续开发前应建立干净 Git 仓库和签名 tag。

---

# 7. 已正确实现并可保留的 Round 7 工作

以下修复不应回退：

1. `AuthoritativeRecordV1`、`OrchestratorAuthorityV1`、transition crate-private；
2. 移除公共 authority-bearing Repository trait；
3. projection digest + latest audit payload digest 复核；
4. subject/turn 中央 lineage gate；
5. Decision selected option/operation/parameter digest → Action 双层校验；
6. successor reverse dependency invalidation closure；
7. terminal Run command-state gate；
8. execution owner/provider caller binding；
9. real operation-level idempotency；
10. Receipt request digest exact binding；
11. exact Outcome ID/revision/digest binding；
12. command ID conflicting bytes 拒绝；
13. sealed record compile-fail；
14. TypeScript/Python generated conformance。

---

# 8. Round 8 推荐实施顺序

本轮不要再扩充 Contract 类型。只修复以下顺序：

## Phase 1：封死 feature 和 trust-domain 旁路

1. `production + test-support` compile fail；
2. 将测试辅助 API 移出 production crate；
3. production/shadow compile-time 分离，或加入不可变 trust domain；
4. Production DB/schema 不能接收 Shadow record。

## Phase 2：统一 Proof subject

1. 创建 `UnsignedCommandEnvelopeV1`；
2. 所有生产 Proof 绑定其 canonical digest；
3. 绑定 run ID、expected version、actor/caller、tenant/scope/purpose/policy 和所有 target refs；
4. 增加 CancelRun 与 HM stale-target attacks。

## Phase 3：修复 Human Model 晋级派生

1. lifecycle 由 Promotion outcome 内部推导；
2. impact 只能保留或收窄；
3. `human-model-write` 成为真实 compile/runtime gate；
4. DB trigger 重复强制 lifecycle/impact；
5. correction proof 绑定 exact assertion ref/revision/digest。

## Phase 4：补齐独立 release gates

对每个 command family 强制：

```text
production_authorization_enabled
production_execution_enabled
human_model_long_term_write_enabled
```

不得只检查 Trust Root 总开关。

## Phase 5：补 P1 语义

- reconciliation → exact Receipt；
- complete Run/Step states；
- provider signed Permit；
- Response delivery receipt；
- production HM query/retention；
- protected Trust Root/anchor。

---

# 9. 下一轮验收门槛

Round 8 至少新增以下测试：

```text
production + test-support => compile fail
production + shadow mode => compile fail or domain-isolation proof
Shadow records rejected by Production
CancelRun proof cannot switch run_id
proof cannot change expected aggregate version
HM correction proof cannot switch assertion revision
HM assertion lifecycle must equal promotion outcome
HM impact cannot exceed source candidate impact
HM write blocked without human-model-write gate
Authorization false blocks AdmitAction
Execution false blocks Reserve/Deliver/Dispatch/Receipt
Reconciliation success creates exact Receipt or stays non-terminal
```

最终门槛仍是：

```text
P0 = 0
security/reliability P1 = 0
two consecutive independent clean reviews
```

Round 7 不能计为第一轮 clean review。
