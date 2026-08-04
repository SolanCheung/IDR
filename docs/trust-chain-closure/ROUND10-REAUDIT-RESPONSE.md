# IDR V1.3 Round 10 response to the Round 9 independent audit

Date: 2026-07-30

Release decision remains:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

This is a remediation candidate for independent review. Passing local tests
does not close an audit finding or authorize Production.

## Incident response for P0-00

The credential-like `coding-plan` value identified by the auditor was confirmed
by SHA-256 fingerprint in two local Aegis configuration files. Both files and
all known Round 9 source-package copies containing that fingerprint were
permanently removed. The independent auditor bundle retains only redacted
metadata. Round 10 packaging uses an explicit source allowlist, rejects
credential/generated/dependency paths and symlinks, and runs a scanner that
emits only path, pattern, length and SHA-256 fingerprint.

This local response is not sufficient to close P0-00. The provider-side key
must be revoked and rotated, downstream usage reviewed, and the replacement
must not be copied into an audit package. No source code or local test can
prove provider-side revocation.

## Round 9 P0 remediation

| Finding | Round 10 remediation | Local attack evidence |
| --- | --- | --- |
| P0-01 expired upstream records remain consumable | All behavioral dependency reads use `require_record_at`/`require_exact_current_record_at` with trusted command time. Historical records are accessible only to an explicit governance invalidate/correct path. | expired Context→Intent, Intent→Decision, Decision→Action, Turn→Action and Response-send tests |
| P0-02 execution rechecks only Exact Authorization | Action Admission stores immutable bindings for Capability, Authority, Policy, ExactAuthorization and ActionAdmission. Reserve/recover/deliver/start load and reverify all five exact proof envelopes against current Trust Root and database time. | rotate away the Policy proof key after Admission; Reserve fails, then succeeds only after fresh Admission under the current root |
| P0-03 mutable execution table releases exactly-once key | Migration 0005 makes reservation identity/exactly-once fields and attempt permit bindings immutable, restricts state changes to legal monotonic transitions and rejects deletion. Startup compares every execution row to the authoritative Run projection. Checkpoints bind a canonical execution-state root. | reservation/attempt identity UPDATE rejection, reservation DELETE rejection, privileged trigger-bypass tamper detected at startup |

The Round 9 P2 time-unit drift is corrected: the V1.3 wire contract uses
integer Unix seconds; PostgreSQL may use higher precision internally.
Checkpoint root/signed fields are immutable and only publication time can move
monotonically.

Migration 0004 now fails before partial alteration when legacy authority,
proof/audit or execution data exists. This deliberately forces verified replay
into a fresh schema; it does not claim a general in-place production migration.

## Unresolved production blockers

The Round 9 P1 control-plane and product-semantic findings remain open,
including transactional Trust Root rotation, post-commit external publication,
protected WORM/KMS anchor and signer, issuer/role separation, provider-facing
signed capability, delivery receipts, historical-proof validation policy,
Outcome aggregation, Human Model lifecycle controls and production migration
evidence. Git provenance and two consecutive clean independent reviews are
also absent.

The release latches and Aegis production compile gate must remain blocked.
