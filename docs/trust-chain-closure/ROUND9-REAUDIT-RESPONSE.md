# IDR V1.3 Round 9 response to the Round 8 independent audit

Date: 2026-07-30

> Historical correction (Round 10): the Round 9 statement that execution
> specialized projections were protected was too broad. Reservation and
> attempt lifecycle rows still allowed identity changes that could release an
> exactly-once key, and the checkpoint did not bind their mutable state. Round
> 10 adds identity-preserving transition guards, deletion denial, full startup
> comparison and an `execution_state_root`.

Release decision remains:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```

This package is a remediation candidate for independent review. Local
validation does not close an audit finding or count as a clean review.

## Round 8 P0 response

| Finding | Round 9 remediation | Local evidence |
| --- | --- | --- |
| P0-01 trust identity absent from signatures | Required `TrustDomainV1` and `environment_ref` now bind Command, unsigned proof target, proof claims/expectation, issuer key, Trust Root, audience, authoritative ref/record, receipt and checkpoint; anchor keys and database uniqueness include environment | Rust golden/conformance foreign-domain and foreign-environment rejection; PostgreSQL identity scans |
| P0-02 delayed expired Authorization | Every Candidate and authoritative record has `valid_from/valid_until`; Admission persists exact Authorization proof ID/expiry and bounds its own validity; reserve/recover/deliver/start reload and reverify that immutable proof with DB time/current Trust Root; execution lease/permit cannot outlive upstream authority | runtime stale-Admission/action-expiry attacks; PostgreSQL vertical execution path |
| P0-03 mutable authoritative content | Migration 0004 makes authority/evidence tables append-only, revokes mutation from `PUBLIC`, and validates Contract identity/validity; startup recomputes every authoritative record digest and cross-checks proof envelopes, current pointers, dependencies, execution, Receipt, Outcome and Human Model projections; checkpoint binds a canonical record-set root | PostgreSQL UPDATE/DELETE rejection; privileged-trigger-bypass tamper detection; vertical specialized-projection mutation attacks |

## Round 8 P1/P2 response

- P1-04 is partially mitigated: startup verifies the database first, publishes a
  missing checkpoint tail, reloads the external anchor, and refuses service if
  the states differ. Publication is still post-commit and lacks a production
  atomic/WORM protocol.
- P1-09 independent-evidence count is now derived from distinct evidence
  references. Sensitive-data lifecycle work remains open.
- P1-10, P1-11 and P1-12 have code remediations: a new Action can follow a
  cancelled old execution, non-Candidate byte/string budgets fail closed, and
  trust/integrity scans cover the authority tables and specialized projections.
- P2-01 and P2-02 are remediated by domain/environment-bearing public refs and
  checked revision arithmetic.
- P2-03 is corrected in the historical Round 8 response and current
  architecture. P2-04 remains open because this source tree has no Git
  provenance.

## Deliberately unresolved production blockers

The following require deployment infrastructure, broader product semantics or
independent external evidence and are not claimed closed:

1. production WORM/KMS checkpoint signer and anchor;
2. protected Trust Root distribution/rotation control service and transactional
   revocation semantics;
3. provider-facing signed capability Permit;
4. Response delivery/fan-out receipts;
5. independently governed issuer registries and separation of duties;
6. multi-observation Outcome conflict/attribution processing;
7. Human Model sensitive-inference retention, decay, deletion and query controls;
8. signed migration/replay tooling, compatibility corpus and long-running
   fuzz/chaos evidence;
9. Git provenance;
10. Aegis Production authorization and two consecutive clean independent reviews.

The release latches and Aegis production compile gate must remain blocked.
