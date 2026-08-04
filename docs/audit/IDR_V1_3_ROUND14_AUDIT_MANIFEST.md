# IDR V1.3 Round 14 final audit baseline manifest

- Audit package date: 2026-07-31
- Baseline frozen: 2026-08-02
- Baseline status: **Final IDR V1.3 Audit Baseline / Production Disabled**
- Archive SHA-256:
  `4c9a3a106a8cd9c61073d6a1642bfe3a40c37fce294e4e1dd86759b797b2dc42`

This final-baseline designation freezes the audited V1.3 bytes. It does not
close the remaining deployment risks and does not authorize Production.

Primary files:

1. `docs/audit/reviews/2026-07-31-round13-independent/IDR-V1.3-round13-package-verification-and-monitoring-2026-07-31.txt`
2. `docs/trust-chain-closure/ROUND14-REAUDIT-RESPONSE.md`
3. `docs/trust-chain-closure/VALIDATION-SUMMARY.md`
4. `tools/resolve_aegis_root.sh`
5. `tools/run_round14_validation.sh`
6. `tools/validate_round14_archive.sh`
7. `tools/build_round14_audit_package.sh`

Required independent sequence:

1. Confirm the outer `.zip.sha256` contains exactly one canonical lowercase
   digest and the ZIP basename only, then verify it.
2. Invoke `validate_round14_archive.sh` against the final ZIP.
3. Confirm extraction occurs under a newly created empty directory.
4. Confirm the packaged validator resolves only the packaged `Aegis-Life`.
5. Confirm all inner SHA256SUMS entries and the complete Rust, PostgreSQL,
   TypeScript, Python and Aegis gates pass.
6. Confirm both `ALL_ROUND14_VALIDATION_STEPS=PASS` and
   `ALL_ROUND14_CLEAN_ROOM_STEPS=PASS` occur exactly once.
7. Re-run with an invalid explicit `IDR_AEGIS_ROOT` and confirm fail-closed
   behavior.
8. Confirm no V1.4 Evidence Qualification or Human Model architecture changes
   were mixed into this delta.

The clean-room log is external evidence for the immutable ZIP, avoiding a
self-referential rebuild after validation.
