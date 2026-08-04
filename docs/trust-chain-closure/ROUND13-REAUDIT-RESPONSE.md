# IDR V1.3 Round 13 response to the Round 12 independent audit

Date: 2026-07-30

Status:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
```

## Independent evidence disposition

The five supplied Round 12 evidence files are preserved byte-for-byte under
`docs/audit/reviews/2026-07-30-round12-independent/` with a local
`SHA256SUMS`.

| Round 12 result | Round 13 disposition |
| --- | --- |
| Package outer/inner integrity | Accepted: all 903 manifest entries passed and archive hygiene was clean. |
| TypeScript 12/12 and typecheck | Accepted. |
| Python 9/9 | Accepted; the spreadsheet-runtime warmup traceback is unrelated environment startup noise and the IDR suite exited 0. |
| Secret scan | Accepted: zero findings. |
| P0 TEMP-table shadowing of `SECURITY DEFINER` functions | Confirmed as a real implementation defect and remediated below. |
| External anchor, Trust Root provider, rotation and deployment evidence gaps | Remain open; no production authorization is claimed. |

## Round 13 closure candidate

Migration
`0008_round13_search_path_and_runtime_identity.sql` is additive and requires
schema version 8 before any Runtime can start.

1. All four `SECURITY DEFINER` functions use the explicit order
   `trusted_schema, pg_catalog, pg_temp`. Runtime has no CREATE authority in
   the trusted schema, and `pg_temp` can no longer precede trusted objects.
2. The legacy v7 opener is revoked from Runtime. The sole Runtime entry point
   is `idr_open_attested_transition_v8`.
3. The v8 opener validates canonical lowercase digests/MACs and compares the
   32-byte HMAC with a fixed 32-iteration XOR/OR comparison before inserting
   the immutable attestation.
4. Production migration revokes database TEMPORARY from PUBLIC. Production
   startup independently rejects any effective Runtime identity that still
   has TEMP through a direct or inherited grant.
5. Production startup requires a real LOGIN that has `MEMBER`, `SET` and
   effective `USAGE` only for the exact schema Runtime role. It rejects an
   identity that can `SET ROLE` to any additional role, assume an object owner,
   create database/schema objects, execute an unexpected schema function or
   observe an unpinned definer path.
6. The owner, Runtime and auditor group roles are explicitly NOLOGIN and
   NOINHERIT.
7. The PostgreSQL attack test creates attacker-owned TEMP versions of both the
   key and attestation tables, inserts an attacker key, computes a matching
   fake MAC and verifies that v8 still rejects it against the owner-only real
   key. It separately proves the Runtime cannot execute v7.

## Claims deliberately not made

Round 13 does not prove provider credential incident closure, production
KMS/HSM rotation, protected Trust Root distribution, WORM checkpoint
publication, provider-signed permits, production delivery, historical Proof
replay, Outcome/Human Model governance, Git provenance, or two clean
independent reviews. Those items remain release blockers.

