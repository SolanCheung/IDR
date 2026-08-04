# IDR V1.3 Round 13 independent re-audit manifest

Audit candidate date: 2026-07-30

Start with:

1. `docs/audit/reviews/2026-07-30-round12-independent/README.md`
2. `docs/audit/reviews/2026-07-30-round12-independent/idr-v13-round12-static-audit-checks.txt`
3. `docs/trust-chain-closure/ROUND13-REAUDIT-RESPONSE.md`
4. `docs/trust-chain-closure/POSTGRES-ROLE-SEPARATION.md`
5. `docs/trust-chain-closure/MIGRATION-GUIDE.md`
6. `docs/trust-chain-closure/VALIDATION-SUMMARY.md`
7. `docs/trust-chain-closure/idr-v13-round13-validation-output.txt`
8. `crates/idr-store/migrations/0008_round13_search_path_and_runtime_identity.sql`
9. `crates/idr-runtime/src/postgres_authority.rs`
10. `crates/idr-store/tests/postgres_vertical_slice.rs`
11. `tools/run_round13_validation.sh`
12. `tools/build_round13_audit_package.sh`
13. `tools/check_audit_package_hygiene.sh`
14. `tools/scan_package_secrets.mjs`

Recommended independent attack order:

- run the exact TEMP-table exploit from the Round 12 static audit using the
  Runtime login and attacker-controlled key material;
- inspect `pg_proc.proconfig` for every `SECURITY DEFINER` function and prove
  `pg_temp` is explicit and after the trusted schema;
- prove v8 is the only schema function executable by Runtime and v7 is denied;
- prove the Runtime LOGIN has no TEMP, CREATE, owner or sequence/base-table
  authority;
- enumerate `pg_has_role(..., 'MEMBER')` and `pg_has_role(..., 'SET')` for all
  roles, then add an unexpected SET-capable membership and verify startup
  fails closed;
- mutate every HMAC byte and verify v8 rejects it; inspect the fixed-work
  comparator and canonical lowercase input gate;
- repeat Round 12 command, Receipt, governance, rollback, package and Aegis
  tests;
- independently confirm provider credential revocation/rotation without
  receiving any old or replacement secret.

Release status remains `PRODUCTION TRUST ROOT = BLOCKED`.
