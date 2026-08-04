# IDR V1.3 Round 3 整改验证

验证日期：2026-07-28  
发布结论：`PRODUCTION TRUST ROOT = BLOCKED`

本文件记录针对 Round 3 严格复审最高优先级问题完成整改后的维护者验证。
它不是独立审计结论，也不替代审计方在独立环境中的复跑。

## 本轮关闭的攻击路径

- Deny Authorization 不能创建 Execution Permit Request。
- Permit lease 不能超过 Exact Authorization 或 Action 的有效期。
- Proof 必须匹配完整 expected kind、issuer、subject ref/digest、tenant、scope、
  purpose 和 policy revision。
- Proof subject digest 覆盖完整 Execution Permit Request，不能改写 provider、
  owner、attempt、lease、dispatch nonce、Authorization 或 idempotency scope。
- `VerifiedProofV1` 不可 Clone。
- Store 在同一个持久化临界区内消费 Proof ID、nonce 和 Authorization ID，重启后
  重放仍被拒绝。
- 不同 Contract Kind 复用同一 UUID 时，依赖失效传播不会被跳过。
- Authorization、Response、Action 和 Proof 使用半开有效期。
- Fast Path Block 使用 `Rejected`，不再被记录为 `Succeeded`。
- TypeScript UUID 输出规范化为 lowercase。

## 验证结果

| 验证项 | 结果 |
| --- | --- |
| IDR Rust tests | 38 passed |
| IDR Clippy `-D warnings` | passed |
| IDR rustfmt check | passed |
| TypeScript Node tests | 10 passed |
| TypeScript typecheck | passed |
| Python unittest | 7 passed |
| Aegis Life adapter tests | 2 passed |
| Aegis adapter Clippy `--no-deps -D warnings` | passed |
| Aegis Life workspace check | passed，host workspace 保留原有 warnings |

TypeScript 的可复现顺序是：

```bash
npm ci
npm test
npm run typecheck
```

审计方记录的 global `tsc` 缺少 `@types/node`，原因是未先安装 lockfile 中的
devDependencies；该输出不能视为源码 typecheck 失败。

## 仍然阻断生产的主要事项

- authoritative Contract sealed issuance；
- Gateway input provenance 与全类型 Proof replay index；
- Admission 全面移除裸 facts；
- claim lease/owner/recovery/reconcile 状态机；
- Permit-bound、provider-signed、multi-attempt Receipt；
- durable Orchestrator；
- signed Outcome Observation 与 Human Model promotion chain；
- 外部不可变 audit checkpoint；
- 跨语言 canonical encoding 与 signature golden vectors。
