# IDR V1.4 Minimal Evidence Governance Conformance Pack

Status: **Specification Only**

This directory converts the object boundaries in
`docs/architecture/IDR-V1.4-EVIDENCE-GOVERNANCE-SPEC.md` into neutral contract
artifacts. It does not implement qualification, snapshot sealing, Decision
issuance, pre-dispatch authority, storage, or database migration.

Contents:

- `idr-evidence-governance-v1.schema.json` defines the closed-loop wire shapes
  with strict object fields and conservative primitive constraints.
- `fixtures/valid/decision-evidence-binding.json` is the first domain-separated
  binding digest golden vector.
- `fixtures/invalid/` contains substitution, warning-drop, unknown-field,
  `DENY` admission, prohibited-personality, and stale-`PASS` cases.
- `manifest.json` records the expected rejection stage and stable reason code.
- `verify-pack.mjs` verifies the fixture integrity without external packages.

Run:

```bash
node contracts/evidence-governance/v1/verify-pack.mjs
```

The schema cannot express every cross-object or authoritative-current-state
invariant. In particular, digest recomputation, tenant/purpose equality,
supersession authority, provenance proof verification, policy allowlists,
trusted time, and atomic dispatch remain mandatory implementation gates. This
pack is intentionally not evidence that V1.4 production implementation is
authorized.
