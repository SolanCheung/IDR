# IDR V1.3 外部审计提交清单

给外部 GPT 或人工审计员时，应提交完整仓库快照，并明确要求审计员运行测试。
不要只提交某一个 Rust 文件或设计说明。

## 必须提交

- `README.md`
- `Cargo.toml`、`Cargo.lock`
- `crates/idr-protocol/Cargo.toml`
- `crates/idr-protocol/src/lib.rs`
- `crates/idr-protocol/src/human_centered/` 全目录
- `crates/idr-protocol/tests/` 全目录
- `crates/idr-runtime/` 全目录
- `crates/idr-store/` 全目录
- `packages/interaction-client/src/`、`tests/`、`vendor/`、`package.json`、
  `package-lock.json`、`tsconfig.json`
- `research/evaluation/src/`、`tests/`、`pyproject.toml`
- `contracts/human-centered/v1/fixtures/` 全目录
- `docs/architecture/` 全目录
- `docs/audit/IDR_V1_3_REMEDIATION.md`
- `docs/audit/reviews/2026-07-28/`（原始复审报告与当时测试输出，仅作为审计证据）
- `docs/audit/reviews/2026-07-28-reaudit/`（第二次复审报告与当时测试输出）
- `docs/audit/reviews/2026-07-28-round3/`（Round 3 严格复审报告与差异记录）
- `docs/audit/reviews/2026-07-29-round4/`（Round 4 严格复审原始报告）
- `integrations/aegis-life/README.md`
- Aegis Life 的 Rust workspace（排除 `target/`、Web 依赖和 `.env`），至少包含
  `crates/idr-aegis-adapter/`、`crates/core-protocol/`、根 `Cargo.toml` 和 `Cargo.lock`

不应提交 `target/`、`node_modules/`、密钥、token、真实用户数据或生产日志。

## 要求审计员执行

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

cd packages/interaction-client
npm ci --offline
npm test
npm run typecheck

cd ../../research/evaluation
PYTHONPATH=src python3 -m unittest discover -s tests -v

cd "/Users/solan/projects/Aegis Life"
cargo test -p idr-aegis-adapter
cargo clippy -p idr-aegis-adapter --all-targets --no-deps -- -D warnings
cargo check --workspace
```

## 审计问题

1. 哪些对象仍可通过反序列化或公开构造器绕过不变量？
2. 哪些 admission 仍依赖调用方可伪造的布尔值或普通引用？
3. Authority、policy、capability 和 human confirmation 是否有可信 issuer、
   签名、用途绑定、有效期和撤销验证？
4. Action authorization 是否绑定精确 contract revision、parameter digest、
   Authority Context，并且只能消费一次？
5. Execution Runtime 是否存在绕过 durable claim/permit 的调用路径？
6. store 在多线程、多进程、崩溃和部分写入下是否保持线性化和可恢复？
7. Decision→Action→Receipt→Outcome→Human Model 的跨对象约束是否在持久化和
   replay 时都重新验证？
8. Rust、TypeScript、Python 是否共享同一 schema、canonical bytes 和 digest vectors？
9. Aegis Life adapter 是否只是 host adapter，还是意外扩大了 IDR 的信任边界？
10. 在全部 P0 关闭前，结论必须保持 `PRODUCTION TRUST ROOT = BLOCKED`。

## 当前复审基线

- 原始完整复审：`docs/audit/reviews/2026-07-28/IDR-V1.3-full-audit.md`
- 原始测试输出：`docs/audit/reviews/2026-07-28/idr-audit-test-output.txt`
- 第二次复审：`docs/audit/reviews/2026-07-28-reaudit/IDR-V1.3-reaudit-report-2026-07-28.md`
- 第二次复审测试输出：`docs/audit/reviews/2026-07-28-reaudit/idr-v13-reaudit-test-output.txt`
- Round 3 严格复审：`docs/audit/reviews/2026-07-28-round3/IDR-V1.3-round3-strict-review.md`
- Round 2→Round 3 文件差异：`docs/audit/reviews/2026-07-28-round3/round2-to-round3-file-diff.txt`
- Round 4 严格复审：`docs/audit/reviews/2026-07-29-round4/IDR-V1.3-round4-strict-review-2026-07-29.txt`
- 整改状态与新增不变量：`docs/audit/IDR_V1_3_REMEDIATION.md`

原始复审材料不得改写为“整改后测试结果”。整改后的验证应重新执行本清单中的命令，
并单独保存输出，避免混淆发现时证据与修复后证据。
