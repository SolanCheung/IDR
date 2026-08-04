# IDR V1.3 Round 14 API visibility

| API/type | Production visibility | Authority |
| --- | --- | --- |
| `CandidateSubmissionV1` | Public, Deserialize | Untrusted candidate only |
| `ProductionProofEnvelopeV1` | Public, Deserialize | Untrusted signed envelope |
| `VerifiedProductionProofV1` | Public opaque read | Trust Root verification only; no Clone, Deserialize or public fields |
| `AuthoritativeRecordRefV1` | Public immutable read | Runtime-issued reference; private fields |
| `AuthoritativeRecordV1` | Crate-private | Concrete PostgreSQL authority only |
| `OrchestratorAuthorityV1` | Crate-private | Owned by `PostgresIdrOrchestratorV1` |
| `evaluate_authoritative_transition_v1` | Crate-private | Concrete PostgreSQL authority only |
| `IdrTransactionalRepositoryV1` | Removed | No downstream repository can borrow authority |
| `PostgresIdrOrchestratorV1::handle` | Public | The only production mutation entry point |
| `connect_shadow` | `shadow-mode` only | Requires `idr_shadow_*`; issues Shadow-domain records |
| `connect_production` | `production` only | Requires `idr_production_*` and every independent release gate |
| `PostgresIdrMigratorV1` | `migration` only | Separate DDL process; mutually exclusive with Production/Shadow Runtime |
| Orchestrator HMAC key | private; protected file input | Opens only transaction-bound authority attestations; never returned or DB-readable by Runtime |
| `idr_open_attested_transition_v8` | Runtime database role only | Sole schema function Runtime may execute; fixed search path and fixed-work HMAC comparison |
| `idr_open_attested_transition_v7` | owner role only | Legacy opener retained for migration history; Runtime EXECUTE revoked |
| PostgreSQL `*_authority_v7` tables | owner role only | Runtime has zero DML and sequence privilege |
| Generic `connect` / `PostgresStartupModeV1` | Removed | Caller cannot select a weaker trust domain at runtime |
| PostgreSQL pool/raw projection | Private | No production pool or raw inspect accessor |
| Human Model/outbox test controls | `shadow-mode + test-support` only | The combination with production is a compile error |
| File Store APIs | `dev-file-store` feature only | dev/test/shadow; incompatible with production |
| TypeScript/Python objects | Public candidate/observation objects | Never Rust authority |

The compile-fail fixtures under `crates/idr-runtime/tests/ui` prove that a
downstream crate cannot import the authority token, authoritative record,
transition function or the removed repository trait. The only public
production state-changing method owns full unsigned-command proof verification,
signed trust-domain/environment enforcement, authoritative validity, database time,
SERIALIZABLE transaction, state transition, projection digest, audit, outbox
and checkpoint publication as one concrete control plane.
