# IDR × Aegis Life Representative Corpus Intake Conformance Pack

Status: **Specification Only / Synthetic Controls Only**

This pack makes the representative-corpus intake boundary executable without
adding a production exporter or reading Aegis data. It verifies strict object
shapes, the structured-facts allowlist, privacy and authority-field rejection,
trusted-time inputs, retention, lineage uniqueness, candidate digests, complete
human-label binding, and the final bundle digest.

Contents:

- `idr-aegis-representative-intake-v1.schema.json`: neutral wire schema;
- `fixtures/valid/synthetic-control-intake.json`: digest-bound golden vector;
- `fixtures/invalid/`: fail-closed governance and substitution cases;
- `manifest.json`: fixed conformance time and expected rejection codes;
- `verify-pack.mjs`: dependency-free offline verifier.

Run:

```bash
node contracts/integrations/aegis-life/representative-corpus-intake/v1/verify-pack.mjs
```

The fixed verification time is fixture input only. A real implementation must
receive trusted current time externally. Passing this pack does not authorize
production export, retention, storage, passive capture, or live mirroring.
