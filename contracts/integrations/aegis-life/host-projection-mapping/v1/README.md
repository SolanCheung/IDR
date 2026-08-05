# IDR × Aegis Life Host Projection Mapping Conformance Pack V1

Status: **Specification Only / Host Source Not Implemented**

This pack locks `AegisHostDecisionSnapshotV1`, the exact 7/4/11 source enums,
their pure mappings to the frozen IDR Host Projection enums, cross-field
consistency, circularity prohibitions, and the fail-closed implementation gate.

Contents:

- `idr-aegis-host-projection-mapping-profile-v1.schema.json`: strict neutral
  profile schema;
- `fixtures/valid/synthetic-host-projection-mapping-profile-v1.json`: synthetic
  golden profile;
- `fixtures/valid/synthetic-host-decision-snapshot-v1.json`: one sealed source
  snapshot;
- `fixtures/valid/synthetic-host-projection-vectors-v1.json`: total enum
  coverage vectors;
- `manifest.json`: schema, golden digests, and negative mutations;
- `verify-pack.mjs`: dependency-free verifier and pure mapper.

Run:

```bash
node contracts/integrations/aegis-life/host-projection-mapping/v1/verify-pack.mjs
```

Passing the pack does not establish an Aegis product source, owner admission,
observation admission, real-data permission, or export approval.
