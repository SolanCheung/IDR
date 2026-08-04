# IDR V1.3 threat model

## Attackers

- malicious/misconfigured host, adapter, model, tool or TypeScript client;
- replaying authenticated caller;
- compromised or revoked issuer/provider key;
- concurrent process racing the same aggregate/idempotency scope;
- worker crashing between claim, dispatch, Receipt or outbox delivery;
- database rollback/fork attacker;
- compromised application database login attempting DDL/trigger bypass;
- payload resource-exhaustion or schema-smuggling attacker;
- code consumer attempting to construct authority types directly.

## Controls and dynamic evidence

| Threat | Control | Test evidence |
| --- | --- | --- |
| Wire authority forgery | private fields, no Deserialize/constructor, capability token | `tests/compile_fail.rs` |
| Actor/caller impersonation | signed principal digest compared with command | vertical-slice impersonation attack |
| Proof replay/revocation | CallerAuthentication plus semantic proof on every command; unique proof ID/nonce; replay Receipt revalidation under current Trust Root/time | PostgreSQL replay/revocation tests |
| Cross-domain/cross-environment replay | signed domain/environment in Command, Proof, Trust Root, key policy, audience, refs and checkpoint key | production conformance foreign-domain/environment attacks |
| Expired upstream authority | every behavioral dependency is current and valid at trusted command time | five direct expired Context/Intent/Decision/Turn/Response attacks |
| Revoked Admission proof after admission | exact five-proof bundle persisted and reverified at every execution transition | Policy-key rotation attack before Reserve |
| Stale CAS/concurrency | `FOR UPDATE`, SERIALIZABLE, expected version | concurrent one-winner test |
| Response mutation/replay | Rust bytes digest, exact channel/audience, nonce uniqueness | vertical-slice tamper and existing TS tests |
| Authorization deny | typed proof assertion must be approve | vertical-slice deny attack |
| Admission bypass/substitution | exact authoritative Admission ref plus Action/provider/owner/idempotency binding and FK/dependency trigger | vertical-slice masquerade and provider-substitution attacks |
| Lost permit/duplicate execution | permanent append-only operation/idempotency Fence, durable attempt, legal recover/expire/reconcile transitions | Runtime-role ACL, privileged pre-restart duplicate and vertical-slice attacks |
| Late Receipt after invalidation | reconciliation preserves dispatch fact | vertical slice |
| Outcome/Human Model promotion forgery | observation/promotion proofs, exact three-object chain and bounded historical-evidence windows | boundary and vertical-slice tests |
| Dependency deletion/cycle | exact FK edges, recursive closure and cycle trigger | Store replay tests + PostgreSQL trigger |
| Outbox worker crash | leased `SKIP LOCKED` claims and owner-bound ack | vertical-slice crash recovery |
| DB rollback | external checkpoint comparison | rollback-behind-anchor test |
| Contract/projection mutation | restricted non-owner Runtime role, append-only/identity guards, runtime digest replay and Fence/Execution cross-check, checkpoint roots | PostgreSQL ACL, tamper/update/delete and privileged trigger-bypass attacks |
| Payload amplification | candidate tree budget plus rendered-byte and idempotency-key limits | runtime resource-budget attacks |

## Residual high-risk boundaries

The current external anchor is an interface with an in-memory shadow
implementation, not a production WORM/KMS backend. Checkpoints are not yet
signed by a protected IDR key. Capability/Policy services are represented by
signed proof assertions but no deployment-grade registry/configuration loader
is included. These are production blockers, not accepted risks.

Migration 0006 defines separate restricted Runtime and read-only auditor group
roles; Production startup rejects a login with DDL, trigger, owner-equivalent
or broad database authority. Triggers remain defense in depth, and startup
detects content drift if a separate privileged operator disables one. A
database owner can still alter schema and data, so Production additionally
requires separately controlled migrator credentials and an independently
protected signed WORM anchor.
