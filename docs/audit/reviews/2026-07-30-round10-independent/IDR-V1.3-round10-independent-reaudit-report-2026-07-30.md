# IDR V1.3 Trust-Chain Closure Round 10 独立复审报告

审计日期：2026-07-30  
审计对象：`IDR-V1.3-trust-chain-closure-round10-2026-07-30.zip`  
外层 SHA-256：`94410cd65d9733b58eb8cc3cbf81c66c0ece422c9e00a7aca66b5f1a561d1685`

## 一、总体结论

```text
ROUND10_FIRST_CLEAN_INDEPENDENT_REVIEW = NO

P0 = 2
P1 = 14
P2 = 4

IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

Round 10 是一次有效整改，不是形式性改动。上一轮三个代码 P0 中：

```text
P0-01 全链时效检查：核心行为链已关闭；历史证据链语义仍有 P1 缺口
P0-02 执行前完整 Proof Set 复核：已关闭
P0-03 Execution 聚合不可变与重放：常规路径已关闭；运行中 privileged tamper 窗口仍为 P0
P0-00 凭据泄漏事件：本地包清理完成，服务商侧撤销/轮换仍未证明
```

因此，Round 10 不能计为第一次“无信任边界 P0”的独立干净复审。

最核心的剩余问题不是 Contract 字段不足，而是 PostgreSQL 控制面仍未把 migration authority 与 runtime authority 分离。当前同一个连接池执行 migration 并处理生产命令；一旦该数据库身份、SQL 执行通道或运维权限被滥用，可以临时禁用 trigger、修改 Reservation 的真实 operation/idempotency identity，并在进程不重启的窗口内释放 exactly-once 唯一槽位。Round 10 能在显式完整性检查或重启时发现，但发现发生在第二次外部副作用之后，不能恢复 exactly-once 保证。

---

## 二、审计范围与独立验证

### 2.1 压缩包完整性

- 外层 ZIP SHA-256 与提交的 `.sha256` 文件一致；
- ZIP 条目：1,052；
- 路径穿越：0；
- 符号链接：0；
- 包内 `SHA256SUMS`：869/869 PASS；
- 独立凭据扫描：0 个命中；
- Round 9 中泄漏的 `.aegis/credentials.json` 路径未出现在 Round 10 包中。

### 2.2 独立动态验证

当前独立环境提供 Node.js、npm 和 Python，但没有 Cargo、rustc 与 PostgreSQL：

- TypeScript 离线安装：PASS；
- TypeScript tests：12/12 PASS；
- TypeScript typecheck：PASS；
- Python tests：9/9 PASS；
- Rust/PostgreSQL：未在本环境独立执行。

包内、经 SHA 清单验证的维护者日志声称：

- IDR default Rust workspace：62 项通过，另有 7 个 compile-fail fixture；
- Shadow PostgreSQL 动态攻击测试：5 项通过；
- Production release-gate profile：14 项通过；
- strict Clippy、rustfmt、生成物 drift、Aegis workspace 均通过。

这些日志是有价值的维护者执行证据，但不能替代本轮独立 Rust/PostgreSQL 动态复现。

---

# 三、已正确实现的部分

## 3.1 上游权威记录的行为时效检查已基本统一

核心行为链现在通过 `require_record_at` 或 `require_exact_current_record_at` 检查：

- current identity；
- trust domain；
- `valid_from <= trusted_now < valid_until`；
- 未被 invalidated。

对应实现位于：

- `IDR/crates/idr-runtime/src/trust_chain.rs:1135-1327`
- `IDR/crates/idr-runtime/src/trust_chain.rs:1417-1418`
- `IDR/crates/idr-runtime/src/trust_chain.rs:2316-2359`

这关闭了 Round 9 中 Context→Intent、Intent→Decision、Decision/Turn→Action 和 Response send 的过期依赖复用。

## 3.2 Action Admission 持久绑定完整五类 Proof

`AdmitAction` 现在保存：

- Capability；
- Authority；
- Policy；
- Exact Authorization；
- Action Admission。

同时将 Admission 的有效期计算为 Action、Authorization 和全部 Proof 到期时间的最小值。证据：

- `IDR/crates/idr-runtime/src/trust_chain.rs:1320-1402`

## 3.3 执行前使用当前 Trust Root 和数据库时间重验证五类 Proof

`ReserveExecution`、`RecoverPermit`、`DeliverPermit` 和 `StartDispatch` 在 PostgreSQL 事务内重新加载五个精确 proof envelope，并使用：

- 当前 Trust Root；
- 数据库可信时间；
- 当前 tenant/scope/purpose/policy；
- exact proof ID 和 expiry；
- proof outcome；

重新验证。证据：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:1872-1960`

这实质关闭了 Round 9 的“只复核 Exact Authorization”问题。

## 3.4 Execution identity 在正常 SQL 权限路径中不可变

Migration 0005 已禁止修改 Reservation 的：

- tenant；
- action/admission identity；
- operation；
- idempotency key；
- provider/owner；
- action/request/parameter digest；
- Authorization identity；
- authority expiry。

Attempt 的 Permit、nonce、lease、provider、owner 同样不可变；状态只能按连续合法迁移更新，删除被 trigger 拒绝。证据：

- `IDR/crates/idr-store/migrations/0005_round10_execution_integrity.sql:20-100`

## 3.5 启动完整性验证明显加强

启动时会：

- 重算 Run projection digest；
- 重算每个 authoritative Contract digest；
- 复核 current pointer；
- 复核依赖、Proof、Receipt、Outcome、Human Model 专用表；
- 复核 record set root；
- 复核 execution state root。

证据：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:717-1187`

## 3.6 Round 10 包卫生整改有效

包构建已改成显式 allowlist，并排除 `.aegis`、credentials、environment、key containers、dependencies 和 runtime state。独立扫描当前 Round 10 ZIP 没有发现凭据样式命中。

但是这只能证明 Round 10 快照干净，不能证明 Round 9 泄漏密钥已经在服务商侧失效。

---

# 四、P0 问题

## P0-00：Round 9 泄漏密钥的服务商侧撤销和轮换仍未证明

### 证据

Round 10 自身明确承认：

> 本地删除不足以关闭 P0-00；仍需在 provider 侧撤销、轮换并检查使用记录。

文件：

- `ROUND10-REAUDIT-RESPONSE.md:19-32`
- `VALIDATION-SUMMARY.md:80-83`

### 当前状态

已完成：

- 本地源文件删除；
- 已知旧包删除；
- Round 10 包卫生修复；
- 当前包独立 secret scan 为 0。

尚未证明：

- 原密钥在服务商控制面已经 revoked；
- 已生成 replacement；
- replacement 没有进入仓库或审计包；
- 泄漏期间无异常调用、费用或数据访问。

### 风险

只要旧密钥仍有效，Round 9 包的任何持有者都可能继续使用它。代码修复和新 ZIP 无法使已泄漏凭据失效。

### 必须修复

提供不暴露新密钥的外部证据：

```text
provider revocation event/reference
rotation completion timestamp
old-key last-used review
billing/usage anomaly review
new-key storage boundary confirmation
```

在外部证据完成前，P0-00 保持 OPEN。

---

## P0-01：运行中的 privileged database tamper 可在完整性检查前释放 exactly-once 槽位

### 代码证据

生产连接使用一个 `PgPool`：

1. 建立连接；
2. 运行所有 migration；
3. 保存为后续 Runtime 命令使用的同一个 pool。

文件：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:321-372`

正常生产 `handle()` 路径只执行：

- release gate；
- command validate；
- `transact_inner()`；
- external anchor publish。

它不会在每次命令前执行 `verify_database_integrity()` 或重新核对全部 execution-state root。文件：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:1964-2001`

完整性验证只在 `connect_for_domain()`→`verify_external_anchor()` 路径运行。文件：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:363-371`
- `IDR/crates/idr-runtime/src/postgres_authority.rs:650-717`

Round 10 自己的攻击测试证明，拥有足够数据库权限时可以：

```sql
ALTER TABLE idr_execution_reservations
DISABLE TRIGGER idr_execution_reservation_update_guard_v5;

UPDATE idr_execution_reservations
SET operation_ref = 'operation:privileged-tamper',
    idempotency_key = 'privileged-tamper';
```

然后完整性检查会发现篡改。文件：

- `IDR/crates/idr-store/tests/postgres_vertical_slice.rs:1449-1488`

问题在于，该测试没有验证“篡改后、完整性检查前”创建第二个原 operation/idempotency Reservation 会被拒绝。

### 可复现攻击序列

前提：攻击者获得当前 IDR 数据库身份、SQL injection 通道或等价 migration/owner 权限。

```text
1. Action A 已创建 Reservation R1：
   operation = operation:pay
   idempotency = invoice-123

2. 禁用 execution reservation update trigger。

3. 将 R1 改为：
   operation = operation:tampered
   idempotency = released-key

4. 恢复或不恢复 trigger。

5. 在 IDR 进程不重启、未调用 verify_database_integrity 的窗口内，
   经正常 Orchestrator 为原 tuple 创建 Reservation R2：
   operation = operation:pay
   idempotency = invoice-123

6. R1 和 R2 均可能到达 Dispatch，造成两次外部副作用。

7. 后续启动或显式 integrity verify 才发现 execution root 不一致。
```

### 为什么这是 P0

当前实现能“事后检测”，不能“在第二次外部副作用前阻止”。对于 exactly-once 和生产 Trust Root，事后发现不等于保持安全不变量。

此外，当前代码没有建立独立 migration role/runtime role：migration 和 Runtime 使用相同 pool，migration SQL 中也没有创建受限 Runtime 角色或收窄 table-owner 权限。

### 推荐修复

必须同时完成：

1. **物理分离数据库身份**
   - migration role：拥有 DDL；不供 Runtime 使用；
   - runtime role：无 `ALTER TABLE`、无 `DISABLE TRIGGER`、无 ownership、无 bypass RLS；
   - read-only auditor role：仅审计读取。

2. **Runtime 不再自动运行 migration**
   - Production 启动只检查 schema version；
   - migration 由独立部署任务完成。

3. **Exactly-once fence 使用数据库不可释放的 append-only identity**
   - 单独 append-only `idr_operation_idempotency_fences`；
   - 原始 `(trust_domain, environment, tenant, operation, idempotency)` 永不 UPDATE/DELETE；
   - Reservation 只引用 fence ID；
   - 即使 Reservation 状态表损坏，fence 仍占用原槽位。

4. **数据库 privilege 回归测试**
   - 使用真实 runtime role 执行 `ALTER TABLE`、`DISABLE TRIGGER`、identity UPDATE、DELETE，必须由 PostgreSQL permission 拒绝；
   - 不应依赖 trigger 自己阻止表 owner。

5. **增加 pre-restart duplicate reservation 攻击测试**
   - Round 10 当前仅测试“篡改后 integrity verify 失败”；
   - 必须测试篡改后第二个原 tuple Reservation 仍无法创建或 dispatch。

---

# 五、P1 问题

## P1-01：幂等重放在当前认证、Trust Root 和时间复核之前返回旧 Receipt

`transact_inner()` 首先按 `command_id` 查询旧 Receipt；只要 envelope 字段和 command digest 相同，就直接返回 `idempotent_replay=true`。此时尚未：

- 获取可信数据库时间；
- 验证当前 CallerAuthentication；
- 读取当前 Trust Root；
- 检查 proof 是否 revoked/expired。

文件：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:1431-1494`

### 风险

捕获到完整旧 command 的主体，可以在认证 Proof 过期或被撤销后继续读取旧 Receipt。它不会重新产生副作用，但属于认证绕过和历史结果重放泄漏。

### 修复

幂等 replay 也必须先验证当前 CallerAuthentication 和 tenant read authorization；或者将 Receipt lookup 拆成受认证的独立只读 API。

---

## P1-02：Receipt→Outcome 没有显式的历史证据有效期规则

`RecordOutcome` 只要求当前 ExecutionReceipt 未 invalidated，没有调用 `require_exact_current_record_at`，因此 Receipt 过期后仍可创建 Outcome。

文件：

- `IDR/crates/idr-runtime/src/trust_chain.rs:1726-1745`

这不一定必然错误：Execution Receipt 可能需要作为历史事实永久可引用。但当前实现和 Round 10 文档“所有 behavioral dependency reads 都检查有效期”的表述不一致，也没有定义：

- 何时允许过期 Receipt 作为历史 evidence；
- Observation time 是否必须落在 Receipt 之后；
- late observation 最大窗口；
- 是否允许在 Receipt 被 superseded 后继续归因。

### 修复

建立独立 helper，例如：

```text
require_historical_evidence_record(
  exact_ref,
  observation_time,
  maximum_observation_delay,
  not_invalidated,
  provenance_policy
)
```

不要在 live authority helper 与 historical evidence helper 之间隐式切换。

---

## P1-03：Outcome→Human Model Candidate 同样没有显式历史有效期策略

`ProposeHumanModelCandidate` 检查 Outcome 是 current 且未 invalidated，但不检查 Outcome 的 `valid_until`。

文件：

- `IDR/crates/idr-runtime/src/trust_chain.rs:1746-1773`

### 风险

长期过期 Outcome 仍可被新的 OutcomeObservation Proof 用于生成认知候选，可能使陈旧行为持续污染 Human Model。

### 修复

定义 Outcome 的认知使用窗口、decay 和 purpose-specific policy，或使用上面的 historical evidence gate。

---

## P1-04：动作失效检测中“修改状态后返回 Err”不会持久化该修改

`ensure_current_execution_action()` 在发现 Action 失效或 authority expiry 时，把内存中的 execution state 改成 `Cancelled` 或 `ReconciliationRequired`，随后返回 `Err(ActionInvalidated)`。

文件：

- `IDR/crates/idr-runtime/src/trust_chain.rs:2412-2448`

PostgreSQL transaction 因错误退出，因此这次状态修改不会提交。正常 successor invalidation 路径会提前持久取消，但如果状态不一致或 expiry 只在 Deliver/StartDispatch 时被发现，系统只能报错，不能原子记录 fence 状态。

### 修复

将“发现失效并持久化 Cancelled/ReconciliationRequired”设计成成功的治理 transition，而不是错误返回路径中的内存副作用。

---

## P1-05：Trust Root 轮换与业务事务没有原子序列化

`current_snapshot()` 由外部 provider 在事务中读取，但 Trust Root 控制面没有与 PostgreSQL command transaction 建立数据库级版本锁或一致性 epoch。轮换与正在执行的命令可能交错。

### 修复

Trust Root version 应成为数据库事务读取并锁定的权威行；proof consumption、Admission 和 execution transition 都绑定同一 `trust_root_epoch`。

---

## P1-06：External anchor 在数据库提交后发布，存在 commit/publish 窗口

`handle()` 先提交数据库事务，再调用 anchor backend；随后再更新 checkpoint 的 `published_at`。

文件：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:1990-2000`

启动时如果数据库 checkpoint 比 anchor 新，代码会主动把数据库 checkpoint 发布到 anchor：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:687-703`

### 风险

如果数据库本身在 commit 后、anchor publish 前被篡改，启动逻辑可能把数据库尾部重新发布为外部 anchor。当前 external anchor 更像复制点，不是独立签名的不可回滚见证。

---

## P1-07：Anchor backend 的 WORM/KMS/独立 signer 仍未实现

Round 10 文档明确把 protected WORM/KMS anchor 与 checkpoint signer 列为未完成项：

- `ROUND10-REAUDIT-RESPONSE.md:51-59`

本地 backend 自己声明 durability，无法证明部署环境中不存在重写、删除或同权限替换。

---

## P1-08：同一数据库身份兼任 migration 和 Runtime

这是 P0-01 的根本控制缺陷：

- `MIGRATOR.run(&pool)`；
- 同一个 pool 被生产 `handle()` 使用。

文件：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:344-365`

Migration 只 `REVOKE ... FROM PUBLIC`，没有建立独立 runtime role。Table owner 不受普通 PUBLIC revoke 提供的安全保证约束。

---

## P1-09：启动时不重新验证历史 Proof 签名

`verify_database_integrity()` 对历史 proof envelope 执行：

- Deserialize；
- `validate_shape()`；
- claims 与列一致性比较。

但没有使用 historical Trust Root 重新验证 Ed25519 signature。

文件：

- `IDR/crates/idr-runtime/src/postgres_authority.rs:921-980`

### 风险

历史 Proof 行若被整体替换并同步更新相关列/roots，系统没有完整的 historical signature replay policy。当前 record/checkpoint root 可提高检测概率，但签名真实性本身没有重放验证。

---

## P1-10：Provider-facing signed Execution Permit 尚未形成部署边界

内部 `ExecutionPermit`/Proof 绑定已加强，但尚未定义可交给外部 provider 的、独立签名且可验证的 capability token，包括：

- exact endpoint/provider；
- operation/parameter digest；
- network audience；
- dispatch fence；
- lease；
- nonce；
- one-time consumption；
- provider-side verification result。

Round 10 文档将 provider-facing signed capability 列为未完成项。

---

## P1-11：Response delivery 和 fan-out Receipt 仍缺失

当前 Response Send Admission 能消费渲染摘要和发送 nonce，但没有真实渠道的：

- accepted delivery receipt；
- provider message ID；
- per-recipient fan-out 状态；
- retry/idempotency；
- revocation-before-send；
- partial failure reconciliation。

因此“允许发送”与“实际发送结果”仍未闭环。

---

## P1-12：Outcome 模型仍过于简化

`validate_outcome_candidate()` 只验证：

- exact receipt digest；
- observed value digest 格式；
- observation 非空。

文件：

- `IDR/crates/idr-runtime/src/trust_chain.rs:2743-2752`

缺少：

- success criteria；
- conflicting observations；
- observer authority；
- causal attribution；
- observation freshness；
- partial outcome；
- environment change；
- reconciliation status。

这与原始 V1.3 Outcome Record 设计仍有明显差距。

---

## P1-13：Human Model 长期治理仍不完整

虽然 Candidate→Promotion→Assertion 的内容派生已显著加强，但仍缺少完整产品和治理能力：

- sensitive inference taxonomy；
- retention class；
- decay execution；
- subject-visible inspection；
- correction/delete SLA；
- legal hold；
- purpose revocation；
- multi-evidence independence，而不是仅按不同字符串计数。

`validate_human_model_promotion()` 当前把独立 evidence 近似为唯一字符串引用数量：

- `IDR/crates/idr-runtime/src/trust_chain.rs:2755-2795`

---

## P1-14：生产迁移、攻击矩阵和独立复审门槛仍未完成

Round 10 migration 对已有 authority data 采取 fresh-schema refusal，而不是提供真实数据迁移和 replay 工具。尚缺：

- 生产历史 corpus migration；
- 跨版本恢复；
- backup/restore 后 Trust Root 和 anchor reconciliation；
- live privileged tamper→pre-restart duplicate reservation 动态测试；
- Receipt/Outcome/Human Model expiry matrix；
- 两轮连续无 P0 独立复审。

---

# 六、P2 问题

## P2-01：Execution 状态机仍与 Master Spec 有命名和粒度漂移

当前聚合状态没有完整区分：

- Reserved；
- PermitIssued；
- Delivered；
- Dispatched；
- Executing；
- AwaitingReceipt；
- FailedRetryable；
- FailedTerminal；
- InvalidatedBeforeDispatch。

部分状态被折叠，增加恢复和审计解释成本。

## P2-02：Secret scanner 仍不是通用供应链扫描器

当前 Round 10 包扫描结果为 0，且已修复上一轮泄漏路径；但自带 scanner 仍基于有限正则、文本文件和大小边界。它不能替代：

- provider secret inventory；
- Git history scanning；
- binary/container scan；
- entropy detector；
- pre-commit/pre-receive policy；
- cloud secret manager audit。

## P2-03：Aegis workspace 仍产生大量既有 warning

这些 warning 不是 IDR 当前 P0，但会降低后续审计信噪比。生产边界 crate 应逐步达到 warnings denied，尤其避免 ambiguous glob re-export 和 unused security-policy code。

## P2-04：IDR 主目录仍无 Git provenance

包内 checksum 能固定当前快照，但无法证明：

- 谁在何时修改；
- 哪个 commit 引入；
- review/approval 历史；
- signed tag；
- clean branch；
- 可重复构建来源。

对于 Production Trust Root，应建立真实 Git 仓库、受保护分支、signed release tag 和 SBOM/provenance attestation。

---

# 七、Round 9 问题关闭矩阵

| Round 9 问题 | Round 10 独立结论 |
| --- | --- |
| P0-00 泄漏凭据 | **OPEN**：本地清理完成，provider revoke/rotation 未证明 |
| P0-01 过期上游仍可消费 | **核心行为链 CLOSED**；Receipt→Outcome→HM 历史证据有效期为 P1 |
| P0-02 执行只复核 Exact Authorization | **CLOSED**：五类 Proof 事务内重新验证 |
| P0-03 可变 Execution 表释放 exactly-once | **PARTIAL**：普通 UPDATE/DELETE CLOSED；运行中 privileged trigger bypass 仍可先造成重复执行、后被发现 |

---

# 八、推荐 Round 11 实施顺序

Round 11 不应新增 Contract 类型，应只处理以下顺序：

## Phase 1：关闭 P0-00 外部事件

```text
revoke leaked key
rotate key
review last-used and billing
record redacted provider evidence
verify replacement never enters source/package
```

## Phase 2：数据库权限与 migration/runtime 物理分离

```text
separate migrator binary and credentials
runtime role cannot ALTER/DISABLE TRIGGER/OWN tables
remove MIGRATOR.run from production connect
schema version fail-closed check only
runtime-role privilege attack tests
```

## Phase 3：建立永不释放的 idempotency fence

新建 append-only identity：

```text
IdempotencyFence {
  trust_domain,
  environment_ref,
  tenant_ref,
  operation_ref,
  idempotency_key,
  first_action_ref,
  created_at,
  fence_digest
}
```

任何 Reservation revision、retry、failure 或 cancellation 都不能修改或删除该 fence。

## Phase 4：补齐历史证据和幂等重放认证语义

```text
current auth required before returning replay receipt
historical evidence helper
receipt-to-outcome time policy
outcome-to-human-model decay policy
```

## Phase 5：重新执行动态攻击

必须新增并通过：

```text
runtime DB role cannot disable trigger
runtime DB role cannot alter identity columns
privileged tamper cannot release idempotency fence
pre-restart second reservation rejected
idempotent replay after caller revocation rejected
expired receipt historical-use policy enforced
expired outcome cognitive-use policy enforced
```

---

# 九、发布门槛

Round 11 完成后仍不能自动解除 BLOCKED。至少要求：

```text
P0 = 0
security/integrity P1 = 0
provider-side credential incident closed
migration/runtime DB role separation demonstrated
pre-restart privileged tamper attack fails closed
idempotency fence is append-only and externally anchored
all Rust/PostgreSQL tests independently reproduced
Aegis production adapter still compile-blocked until review
first clean independent review = PASS
second consecutive clean independent review = PASS
```

## 最终判断

Round 10 已经把 IDR 从“明显存在普通代码旁路”推进到“主要正常路径可信、剩余问题集中在数据库控制面、部署信任和产品闭环”的阶段。这是实质进步。

但 IDR 的目标是生产信任根。只要同一个数据库身份仍同时拥有 migration/DDL authority 和 Runtime state authority，trigger 与 startup integrity 就只能提供事后检测，不能保证外部副作用发生前 fail closed。因此本轮仍不能通过第一次独立干净复审。
