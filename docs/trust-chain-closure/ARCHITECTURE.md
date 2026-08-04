# IDR V1.3 Trust-Chain Closure architecture

Status: Frozen IDR V1.3 Round 14 audit baseline; Production disabled.
`PRODUCTION TRUST ROOT = BLOCKED`.

## Authority flow

```text
TypeScript / Aegis / model / provider
  -> exact domain/environment-bound IdrCommandEnvelopeV1 without proofs
  -> signed ProductionProofEnvelopeV1 bound to its unsigned digest
  -> connect_shadow + idr_shadow_* schema
     OR connect_production + idr_production_* schema
  -> PostgresIdrOrchestratorV1::handle
       private PgPool + private OrchestratorAuthorityV1
       SERIALIZABLE transaction
       DB clock + aggregate FOR UPDATE + domain-specific current Trust Root
       -> exact proof set, full command target, signature, principal, context,
          trust domain, environment, time and revocation
       -> current CallerAuthentication plus command-specific authority
       -> exact replay re-authentication before stored Receipt return
       -> execution-time re-verification of the exact stored Admission proof bundle
       -> proof ID/nonce replay check
       -> runtime semantic transition
       -> HMAC Orchestrator attestation bound to txid/backend/command/Run/versions/digests
       -> Runtime-writable authority view
       -> owner-only authority base table
       -> record/current/dependencies/invalidation
       -> execution/outcome/human-model projection
       -> run event + step snapshot + audit hash chain + outbox + receipt
  -> immutable receipt/read model
```

The concrete PostgreSQL Orchestrator owns the crate-private authority,
transition engine, attestation key and pool. There is no public Repository
authority trait or generic Orchestrator. Public wire types express candidates
and proof envelopes only. `AuthoritativeRecordV1` itself is crate-private.
Compile-fail tests prove
downstream code cannot import the record, authority, transition function or
the removed repository trait.

## Language and host boundaries

| Boundary | May do | Must not do |
| --- | --- | --- |
| Rust protocol | Parse candidates, canonicalize, verify Ed25519 proofs | Accept authoritative state from wire |
| Rust runtime | Decide transitions and issue authoritative records | Trust TypeScript/Python decisions |
| PostgreSQL authority | Reverify with DB time/current Trust Root and commit atomically | Expose a writable pool or dispatch before `DispatchStarted` commits |
| TypeScript | Validate generated wire shapes, render product UI | Recompute or override Rust authority |
| Python | Offline evaluation and conformance | Write production state |
| Aegis Life | Use the shadow adapter as a reference host | Gain IDR production authority |

`production + dev-file-store`, `production + test-support`, and
`production + shadow-mode` are compile errors. Shadow and Production have no
caller-selectable runtime mode and accept only their own schema prefix and
persisted trust domain. A production-only Store build
uses `--no-default-features --features production`. Aegis
`aegis-production-adapter` is separately compile-blocked.

Production migration authority is a different compile profile and executable.
`production + migration` and `shadow-mode + migration` are compile errors.
The Production Runtime only verifies the exact schema version and its
restricted database role; it never applies migration DDL. See
`POSTGRES-ROLE-SEPARATION.md`.

## PostgreSQL atomic boundary

One command transaction verifies the stored trust domain and projection digest
against the latest audit payload. It then authenticates the exact semantic
transition with a transaction-bound HMAC before writing through the 22
authority views. Runtime has no DML privilege on the owner-only
`*_authority_v7` tables and no sequence privilege.

Migration 0008 pins every `SECURITY DEFINER` function to
`trusted_schema, pg_catalog, pg_temp`. Because Runtime cannot create in the
trusted schema and the temporary schema is explicitly last, attacker-owned
TEMP tables cannot shadow the attestation key or immutable attestation table.
The sole Runtime opener is v8, which performs a fixed-work 32-byte HMAC
comparison. Production startup also rejects effective TEMP privilege and any
unexpected SET-capable role membership.

The attested transaction writes:

- aggregate CAS projection and step snapshot;
- authoritative record/current pointer and exact dependency edges;
- recursive invalidation closure;
- proof envelopes and unique proof/nonce consumptions;
- response-send or execution specialized state;
- an append-only operation/idempotency Fence independent of execution
  lifecycle rows;
- Receipt, Outcome and Human Model specialized projections;
- run event, audit event/hash, outbox event, command receipt and checkpoint.

The Receipt contains the immutable attestation ID and its exact canonical
content is protected by a second `receipt_mac`. Replay and startup recompute
both HMACs and bind the Receipt to the corresponding Run event, audit event
and checkpoint. Bound Action Admission proof revocation creates a
dedicated governed transition instead of losing the evidence to transaction
rollback.

Database triggers independently enforce trust-domain/environment isolation,
append-only authoritative tables, immutable lineage, Contract validity,
contiguous Contract revisions, dependency acyclicity, Decision-to-Action
derivation, Action Admission linkage, exact Authorization/expiry bindings,
immutable execution identity/exactly-once keys, legal contiguous execution
transitions, exact request digest, and Human Model materialization.
Startup integrity verification recomputes every authoritative record digest,
cross-checks specialized projections and current pointers, validates stored
proof envelopes, compares execution reservations/attempts with the
authoritative Run projection and permanent Fence, and checks both checkpoint
record-set and execution-state roots. The execution-state root includes Fence,
Reservation and Attempt material.

External checkpoint publication remains outside the database commit. Startup
republishes a missing verified tail before serving and rejects a fork or
anchor-ahead state. A production WORM/KMS signer/backend is still not shipped,
so this recovery protocol does not authorize Production.

## Cross-language source

`crates/idr-protocol/examples/generate_production_contracts.rs` generates the
JSON Schema, TypeScript types/validators, Python evaluation DTO and the shared
canonical/digest/signature golden vector under `contracts/production/v1`.
The constrained JCS profile rejects floats and integers outside the JavaScript
safe range; property ordering uses raw UTF-16 code units.
