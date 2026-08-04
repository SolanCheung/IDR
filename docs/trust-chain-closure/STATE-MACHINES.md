# IDR V1.3 Round 14 state machines

## Run and command gate

Every command first passes a central Run-state gate. `Cancelled`, `TimedOut`
and other terminal states cannot reserve, deliver or dispatch execution.
Outcome/Human Model post-processing is explicitly allowed only for the
non-cancelled states where immutable execution evidence already exists. Every
accepted command advances the aggregate and event sequence exactly once.
Ordinary rejected commands write nothing. A bound Admission proof that becomes
invalid is intentionally different: it writes a dedicated governance event,
invalidates the Admission and returns to `WaitingAuthorization` before
Dispatch or enters `ReconciliationRequired` after Dispatch.

Persisted Step snapshots derive `waiting`, `failed`, `cancelled` or `completed`
from the authoritative Run state and execution event. Permit-issued,
permit-delivered and dispatch-started are persisted as waiting, not completed.

## Execution attempt

```text
PermitIssued -> PermitDelivered -> DispatchStarted
     |                |                 |
     +---- Expired ---+                 +-> ReconciliationRequired
     |                                  |          |
     +---- Cancelled (pre-dispatch)      |          +-> exact Receipt only
                                        +-> Receipt -> Succeeded/Failed/
                                                       Rejected/Compensated
```

Rules enforced by runtime and PostgreSQL:

- An append-only Fence permanently owns the selected real operation and
  `(trust domain, environment, tenant, operation, idempotency key)` scope.
  Reservation lifecycle rows reference that Fence and cannot release it.
- Reserve/recover/expire require the bound owner identity; deliver/start,
  provider Receipt and reconciliation require the bound provider identity.
- Every lifecycle command requires a current CallerAuthentication proof plus
  its specialized verified proof kind.
- Action and Admission must be current and unexpired; all five Capability,
  Authority, Policy, ExactAuthorization and ActionAdmission proofs accepted at
  Admission are loaded from the immutable proof store and rechecked against
  current trusted time and Trust Root before reserve/recover/deliver/start.
- Permit validity and lease cannot outlive Action, Admission, Authorization or
  the current Execution Permit proof.
- Retry for the same Action reuses the Reservation, increments attempt by
  exactly one and issues a new permit/nonce; only Failed, Rejected or Expired
  can retry. A newly authoritative Action may create a fresh Reservation after
  the old Action's execution was cancelled.
- Reservation identity, Action/Admission binding, provider/owner,
  operation/idempotency key, digests and validity bounds are immutable after
  insert. Attempt permit ID, nonce, lease, provider and owner are immutable.
  PostgreSQL permits only explicit monotonic lifecycle transitions and rejects
  deletion, so a failed execution cannot release an exactly-once slot.
- Action invalidation before dispatch cancels; after dispatch it enters
  reconciliation. Discovery during delivery/dispatch is a successful
  governance transition, so the cancelled/reconciliation state is committed.
- Revocation or expiry of a proof stored in the bound Admission bundle follows
  the same governed path; it cannot disappear as a rolled-back error.
- Receipt request digest is derived from exact Action revision, operation and
  parameter digest.
- `ReconcileExecution(Unknown)` preserves the reconciliation state. An enum
  value alone can never create terminal truth; only the exact Receipt path can.
- A terminal Receipt also moves the Run to Succeeded, Failed or Rejected.

## Human Model

```text
Outcome -> Candidate -> Promotion Decision -> materialized Assertion
                                             |
                                             +-> correction/rejection/deletion
```

Candidate and Promotion carry the exact Outcome record ID, revision and digest.
Receipt evidence can create an Outcome only before its explicit 86,400-second
observation deadline. Outcome evidence can create a Human Model Candidate only
before its explicit 604,800-second cognitive-use deadline. Exact replacement
and invalidation checks still apply.
The Assertion request carries only the exact source Candidate digest.
Lifecycle is derived from the Promotion outcome and maximum impact from the
source Candidate. Predicate, value, scope, evidence, purposes, Outcome binding
and exact Promotion ID/revision/digest are materialized internally and checked
again by a PostgreSQL guard.

The minimum independent evidence count is derived from distinct
`evidence_refs`; a caller-supplied count must equal the derived value and
cannot promote a duplicate evidence root.
