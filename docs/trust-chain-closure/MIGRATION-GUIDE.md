# Migration guide: pre-production Round 12 to Round 13

1. Treat migrations 0004, 0006 and 0007 as pre-production replay boundaries.
   Migration 0007 refuses any existing authority row because legacy rows have
   no Orchestrator attestation. Replay independently verified history into a
   fresh environment-specific schema; never synthesize attestations or disable
   the preflight.
2. Apply migrations 0001 through
   `0008_round13_search_path_and_runtime_identity.sql` in order. Runtime
   accepts only the complete version-8 schema.
   For an existing pre-production version-7 schema, first revoke the generated
   Runtime group from every LOGIN and terminate its sessions. Apply 0008 with
   the isolated migrator, verify the version-8 ACL/path checks, and only then
   grant the Runtime group to a clean application LOGIN. Never expose the
   interval between migrations 0007 and 0008 to an application identity.
3. Generate a random 32-byte Orchestrator HMAC key. Store it as 64 lowercase
   hexadecimal characters in a secret-manager-mounted mode-0600 file. Do not
   put key bytes or the database URL in process arguments, source, audit logs
   or the re-audit package.
4. Run the migration-only executable:

   ```bash
   IDR_MIGRATOR_DATABASE_URL='postgresql://...' \
   IDR_ORCHESTRATOR_ATTESTATION_KEY_FILE=/run/secrets/idr-orchestrator-hmac \
   cargo run \
     --manifest-path tools/idr-production-migrator/Cargo.toml \
     --locked --release -- \
     idr_production_example
   ```

5. Use a dedicated IDR database. Migration 0008 revokes TEMPORARY from PUBLIC.
   Assign distinct LOGIN roles only to the generated schema-specific
   SHA-256-derived `idr_runtime_*` and `idr_auditor_*` roles. Runtime and
   auditor must not inherit the `idr_owner_*` role, migrator identity or any
   other role they can activate with SET ROLE.
6. Verify Runtime has:

   - SELECT/INSERT/UPDATE only on the 22 authority views;
   - SELECT only on `_sqlx_migrations` and immutable attestations;
   - EXECUTE only on `idr_open_attested_transition_v8`, with v7 denied;
   - zero DML on real tables, zero sequence privilege and no key-table access;
   - zero effective TEMPORARY privilege and no unexpected MEMBER/SET path;
   - an explicit trusted `SECURITY DEFINER` search path with `pg_temp` last.

7. Mount the same key file into the Rust Runtime. Production construction still
   fails closed while release gates remain blocked.
8. Submit mutations only through `PostgresIdrOrchestratorV1::handle`.
   Candidates, TypeScript, Python, Aegis and a raw Runtime database login
   cannot manufacture a valid authority HMAC.
9. Every command carries current CallerAuthentication plus its exact
   command-specific proof set. Receipt replay revalidates proofs and its stored
   Orchestrator attestation.
10. If a bound Action Admission proof expires or is revoked before a later
    execution transition, consume the attempted command into the explicit
    governance path: invalidate the Admission and persist cancellation before
    Dispatch or reconciliation after Dispatch.
11. Run `tools/run_round14_validation.sh` and independently inspect the
    Production migration, exact ACLs, forged-row attacks, key-file handling,
    TEMP-table shadow exploit, fixed-work MAC validation, attestation replay,
    proof-revocation governance and external checkpoint behavior.
    For the release artifact, run `tools/validate_round14_archive.sh` against
    the final ZIP so the packaged IDR and packaged `Aegis-Life` are the exact
    files exercised.
12. Do not remove the Production startup latch or enable the Aegis Production
    adapter. Provider credential incident closure, protected Trust Root/anchor,
    provider-facing Permit and remaining P1 gates are still open.

Once any numbered migration is deployed to an authorized environment, freeze
it and make every subsequent schema change additive.
