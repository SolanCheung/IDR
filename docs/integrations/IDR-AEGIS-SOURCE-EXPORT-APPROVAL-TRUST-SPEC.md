# IDR × Aegis Life Source Export Approval and Trusted-Time Specification

Status: **Specification Only / Real-Data Export Blocked**

## 1. Purpose and authority boundary

This specification defines the authorization proof and trusted-current-time
checks that must precede any future projection of an Aegis source into the IDR
offline representative-corpus boundary. It does not implement an exporter,
create consent or lawful basis, grant product-data access, or authorize a
production issuer or clock.

The only positive conformance path is `synthetic_control`. A conformance result
of `ALLOW_SYNTHETIC_CONTROL_ONLY` is not transferable to product, staging,
support, telemetry, conversation, journal, memory, or database data.

The existing `approval` field in `AegisRepresentativeIntakeV1` is a downstream
consumption summary. It is not an authorization proof and MUST NOT be used as
the source of issuer identity, current time, revocation state, lawful basis, or
scope.

## 2. Closed object set

The trust check consumes exactly these three sealed objects:

```text
AegisRepresentativeExportApprovalV1
TrustedCurrentTimeEvidenceV1
AegisRepresentativeExportVerificationRequestV1
```

It returns one of:

```text
ALLOW_SYNTHETIC_CONTROL_ONLY
DENY
```

No warning or best-effort path exists. Unknown fields, missing external state,
unsupported algorithms, stale time, unavailable revocation state, scope
wildcards, or digest drift return `DENY`.

## 3. `AegisRepresentativeExportApprovalV1`

The approval binds one issuer decision to one source version, exporter,
purpose, tenant set, actor set, source class set, exact downstream contracts,
volume limit, validity interval, and revocation registry revision.

Required fields are defined by
`idr-aegis-representative-export-approval-v1.schema.json`. In particular:

- `approval_id` is immutable and globally unique within the issuer domain;
- `source_pin` fixes the reviewed Aegis repository, commit, and tree;
- `exporter_ref` identifies one future implementation, never a wildcard;
- `purpose` is exactly `idr_offline_representative_evaluation`;
- tenant and pseudonymous actor allowlists are non-empty, unique, and contain
  no `*`, `all`, prefix, pattern, or regular-expression entry;
- `allowed_source_classes` is explicit; the conformance approval contains only
  `synthetic_control`;
- `contract_bindings` binds the structured-observation schema, ingress-source
  contract, producer registry, Host Projection mapping profile, and intake
  schema by exact SHA-256 or sealed object digest;
- `data_policy` forbids raw content, credentials, Human Model data, personality
  inference, model training, secondary use, and onward transfer;
- `valid_from_ms` is inclusive and `valid_until_ms` is exclusive;
- `revocation_binding` fixes a registry, revision, and fail-closed policy;
- `assurance` separates `SYNTHETIC_ONLY` from `PRODUCTION_APPROVED`.

`PRODUCTION_APPROVED` requires a signature from a separately configured trust
store and an issuer authorized for the precise tenant, source, and purpose. No
such issuer, trust store, key, signature suite, or approval is provided here.
The synthetic fixture uses `SYNTHETIC_ONLY` and can never authorize a
non-synthetic source.

## 4. `TrustedCurrentTimeEvidenceV1`

Current time MUST be injected from outside the export bundle. A source record,
approval, request, filesystem timestamp, local process clock, or bundle
manifest is not authoritative.

The evidence binds:

- one externally configured `clock_source_ref` and `clock_profile_version`;
- a monotonic `clock_epoch` and `sequence`;
- an unpredictable request `nonce` and the fixed export-verification purpose;
- `attested_time_ms` plus `uncertainty_ms`;
- an assurance class and signature metadata;
- a domain-separated `time_evidence_digest`.

The verifier computes the closed interval
`[attested_time_ms - uncertainty_ms, attested_time_ms + uncertainty_ms]`. The
entire interval MUST be contained in the approval interval. Reuse of a nonce,
rollback of epoch/sequence, an unrecognized clock, expired clock profile,
excessive uncertainty, or absent signature verification fails closed.

The committed fixture is `SYNTHETIC_ONLY`. It is valid only inside this
conformance pack and is not evidence that Aegis has a production trusted clock.

## 5. `AegisRepresentativeExportVerificationRequestV1`

The request binds one attempted export to:

- the exact approval and time-evidence digests;
- one source class, source pin, exporter, purpose, tenant, and actor;
- the same five contract digests sealed by the approval;
- a bounded candidate count;
- one exact revocation registry snapshot and checked revision;
- a lawful-basis disposition;
- prohibited-use booleans fixed to `false`;
- a unique request nonce and domain-separated request digest.

For `synthetic_control`, `lawful_basis.status` is
`NOT_REQUIRED_SYNTHETIC` and no claim about product-data legality is made. Any
non-synthetic request requires a separately reviewed lawful-basis proof bound
to the same tenant, actor or approved cohort, purpose, source, validity period,
and withdrawal state. Because that proof format and authority are unresolved,
all non-synthetic requests are `DENY` in this phase.

## 6. Hashing and version binding

All strings are NFC. Integers are non-negative JSON safe integers. Unknown
fields are rejected. Digests use lowercase hex SHA-256 over canonical JSON:

```text
SHA-256(ASCII(domain) || 0x00 || UTF8(canonical_json(object_without_digest)))
```

Domains are:

```text
idr:aegis:representative-export-approval:v1
idr:aegis:trusted-current-time-evidence:v1
idr:aegis:representative-export-verification-request:v1
```

Canonical JSON recursively sorts object keys, preserves array order, emits no
insignificant whitespace, and accepts no floating point numbers. Any schema,
source commit/tree, policy, registry revision, exporter, scope, validity,
artifact, or prohibited-use change creates a new digest and invalidates prior
requests.

## 7. Revocation and supersession

Revocation is checked against an external, authenticated registry selected by
`revocation_registry_ref`. The verifier MUST require a registry revision at
least as new as `minimum_registry_revision`, require the request to bind the
exact checked revision, and reject `REVOKED`, `UNKNOWN`, unavailable, stale, or
rollback state.

A later approval for the same scope does not silently replace an earlier one.
Supersession must explicitly name the superseded approval; both revocation and
supersession invalidate unexecuted requests. Revocation propagates to queued
exports, projected-but-not-admitted candidates, intake bundles, and any
pre-evaluation staging copy. Deletion and retention execution remain separate
unresolved gates.

## 8. Fail-closed verification order

Before reading or projecting source data, a future product-boundary verifier
MUST perform, in order:

1. parse strict versioned shapes and reject unknown fields;
2. verify all three domain-separated digests;
3. load approval issuer and clock trust anchors from external configuration;
4. verify assurance class and cryptographic signatures;
5. match exact source pin, exporter, purpose, tenant, actor, source class, and
   contract bindings with no wildcard semantics;
6. verify the externally supplied time interval is wholly within approval
   validity and enforce clock nonce/epoch/sequence replay protection;
7. fetch and authenticate current revocation state, reject rollback, and prove
   the approval is not revoked or superseded;
8. verify bounded volume, lawful basis, and every prohibited-use flag;
9. immediately before source read, repeat time and revocation validation;
10. return `ALLOW_SYNTHETIC_CONTROL_ONLY` only for the isolated synthetic path;
    otherwise return `DENY` until the remaining gates are implemented and
    independently approved.

Validation success MUST be transaction-local and single-use. It may not be
cached as a reusable bearer authorization. Any wait, queue transfer, retry,
process restart, clock-profile change, policy revision, approval update,
revocation update, source-tree change, or contract change requires a fresh
request and fresh trusted-time evidence.

## 9. Responsibility boundary

- Rust is the future authority boundary for digest/signature validation,
  replay protection, revocation/time checks, single-use authorization, and
  fail-closed source-read gating. No Rust change is authorized in this phase.
- TypeScript may collect opaque proofs and present denials, but may not mint an
  approval, trust local time, widen scope, recompute authorization, or bypass a
  Rust denial. No TypeScript change is authorized in this phase.
- Python may generate synthetic negative cases and evaluate already admitted
  offline data. It may not access product sources, issue approvals, provide
  authoritative time, or weaken admission. No Python change is authorized in
  this phase.
- Aegis Life remains responsible for controller authorization, lawful basis,
  source access, least privilege, retention, deletion, and audit evidence. No
  Aegis runtime change is authorized in this phase.

## 10. Gate effect and exclusions

This specification changes only two readiness labels:

```text
SOURCE_SPECIFIC_EXPORT_APPROVAL = SPECIFIED_NOT_IMPLEMENTED
TRUSTED_EXPORT_AND_VERIFICATION_CLOCK = SPECIFIED_NOT_IMPLEMENTED
```

It deliberately leaves:

```text
CONSENT_OR_LAWFUL_BASIS = UNRESOLVED
REPRESENTATIVE_EXPORTER = BLOCKED
REAL_DATA_ACCESS = NOT_AUTHORIZED
```

It adds no production code, dependency, migration, real data, passive capture,
live mirror, Setoka, personality inference, vector database, graph memory,
semantic retrieval, or Human Model functionality.
