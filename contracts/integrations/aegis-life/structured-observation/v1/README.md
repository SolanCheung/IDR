# IDR × Aegis Life Structured Observation Conformance Pack V1

Status: **Specification Only / Synthetic Controls Only**

This pack locks the neutral wire shape for `AegisStructuredObservationV1`, its
receipt and family-specific assertion digests, observation seal, and pure
projection to the 42-leaf `ShadowReplayEnvelopeV1` allowlist.

Contents:

- `idr-aegis-structured-observation-v1.schema.json`: strict neutral JSON Schema;
- `fixtures/valid/synthetic-structured-observation-v1.json`: synthetic golden
  observation;
- `manifest.json`: fixed logical projection time, expected golden digests, and
  manifest-driven negative mutations;
- `verify-pack.mjs`: dependency-free offline verifier and pure projector.

Run:

```bash
node contracts/integrations/aegis-life/structured-observation/v1/verify-pack.mjs
```

Passing the pack means only that the specification is internally consistent.
It does not prove that an Aegis producer exists, authorize real-data access, or
permit an exporter, passive capture, live mirror, storage, or production use.
