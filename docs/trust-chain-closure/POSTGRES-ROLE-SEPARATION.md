# PostgreSQL migrator, Orchestrator and auditor separation

Status: Frozen IDR V1.3 Round 14 audit baseline; Production disabled.
`PRODUCTION TRUST ROOT = BLOCKED`.

## Authority boundary

Production uses three schema-specific NOLOGIN/NOINHERIT group roles plus a
separate migrator login. Group names derive from the first 128 bits of SHA-256
over the exact schema name:

| Identity | Required authority | Forbidden authority |
| --- | --- | --- |
| migrator login | create the schema, apply numbered DDL and provision the Orchestrator key | application Runtime use |
| `idr_owner_*` | own authority base tables, sequences and SECURITY DEFINER functions | LOGIN |
| `idr_runtime_*` | SELECT and attested INSERT/UPDATE through the 22 `idr_*` views; SELECT immutable attestations | every base-table DML, sequence use, key reads, DDL, TRIGGER, owner membership |
| `idr_auditor_*` | SELECT views and immutable attestations | key reads, base-table access, writes, DDL and ownership |

Migration 0007 renames each authority table to
`*_authority_v7`, creates an identically shaped view at the original name and
installs one owner-executed bridge trigger. The Runtime role has zero DML on
all real tables. Before the bridge accepts a write,
`idr_open_attested_transition_v8` must authenticate an HMAC that binds:

- trust domain and environment;
- purpose, attestation ID, command ID, Run and tenant;
- pre/post aggregate versions;
- command and transition digests;
- database transaction ID, backend PID, nonce and key version.

The HMAC key is provisioned from a protected mode-0600 file. Its bytes are kept
in an owner-only table that Runtime and auditor roles cannot read. Capturing an
attestation does not make it reusable: the bridge requires the same database
transaction and backend PID and the nonce/attestation rows are unique and
immutable.

Migration 0008 pins all owner-executed functions to
`trusted_schema, pg_catalog, pg_temp`, revokes Runtime EXECUTE from the v7
opener and grants only v8. The v8 opener requires canonical lowercase
digests/MACs and compares the decoded 32-byte MAC using fixed work. In a
dedicated Production database the migration revokes TEMPORARY from PUBLIC.
Startup independently rejects the application LOGIN if TEMP is restored
through another grant.

Production Runtime never calls `MIGRATOR.run`; it checks exactly migration
version 8 and fails startup unless the effective ACL is exactly the view-only
profile above. The login must have effective `MEMBER`, `SET` and `USAGE` for
the exact Runtime group and no SET-capable membership in any other role. It
must not have TEMP, database/schema CREATE, owner, base-table, sequence or
unexpected function authority. Runtime also recomputes every persisted
attestation HMAC and binds each command Receipt to its Run event, audit event
and checkpoint.

## Deployment sequence

1. Create a fresh `idr_production_*` schema with the dedicated migrator login.
   Migration 0007 deliberately refuses a non-empty authority schema; verified
   pre-production history must be replayed into a fresh schema.
2. Place a random 32-byte key as 64 lowercase hexadecimal characters in a
   secret-manager-mounted mode-0600 file.
3. Run the separately compiled migrator. The database URL and key never appear
   in process arguments:

   ```bash
   IDR_MIGRATOR_DATABASE_URL='postgresql://...' \
   IDR_ORCHESTRATOR_ATTESTATION_KEY_FILE=/run/secrets/idr-orchestrator-hmac \
   cargo run \
     --manifest-path tools/idr-production-migrator/Cargo.toml \
     --locked --release -- \
     idr_production_example
   ```

4. Obtain the generated role names:

   ```sql
   WITH digest AS (
     SELECT substr(
       encode(sha256(convert_to('idr_production_example', 'UTF8')), 'hex'),
       1,
       32
     ) AS value
   )
   SELECT
     'idr_owner_' || value AS owner_role,
     'idr_runtime_' || value AS runtime_role,
     'idr_auditor_' || value AS auditor_role
   FROM digest;
   ```

5. Use a dedicated IDR database. Grant only `idr_runtime_*` to the application
   LOGIN and only `idr_auditor_*` to the audit LOGIN. Neither login may be a
   member of the migrator, owner or any other SET-capable role. Do not restore
   PUBLIC TEMPORARY.
6. Mount the same HMAC key file into the separately compiled Rust Runtime.
   The current release latch still prevents Production startup until all
   independent release gates are authorized.

## Required independent evidence

- Provider-side key generation, storage ACL, rotation and deletion evidence.
- Distinct migrator, Runtime, owner and auditor identities.
- Zero Runtime DML on every `*_authority_v7` table and zero sequence privilege.
- Runtime cannot SELECT `idr_orchestrator_attestation_secrets_v7`.
- Runtime can execute only `idr_open_attested_transition_v8` among schema
  functions.
- Runtime LOGIN has no TEMP and no unexpected MEMBER/SET path.
- Every `SECURITY DEFINER` function has an explicit trusted path with
  `pg_temp` last.
- Attacker-owned TEMP key and attestation tables plus a matching fake MAC are
  rejected.
- Raw Runtime INSERT/UPDATE attempts for Run, Contract, Receipt, Fence and
  attestation rows fail closed.
- An authentic command commits through the view boundary and survives replay
  and restart verification.
- Auditor can SELECT views and cannot write.

The local PostgreSQL suite proves the migration and ACL behavior in this
snapshot. It does not prove real deployment identities, KMS/WORM controls or
provider-side credential revocation.
