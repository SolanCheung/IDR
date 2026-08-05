# IDR × Aegis Life Producer Trust Admission Conformance Pack V1

Status: **Specification Only / All Producers Not Implemented**

This pack locks the five fact-producer responsibility profiles, their exact
31-leaf ownership, typed-input allowlists, prohibited feedback sources,
required proofs, and the fail-closed implementation gate.

Contents:

- `idr-aegis-producer-trust-registry-v1.schema.json`: strict neutral schema;
- `fixtures/valid/synthetic-producer-trust-registry-v1.json`: synthetic golden
  registry with all producers `NOT_IMPLEMENTED`;
- `manifest.json`: expected digests and manifest-driven negative mutations;
- `verify-pack.mjs`: dependency-free offline verifier.

Run:

```bash
node contracts/integrations/aegis-life/producer-trust-admission/v1/verify-pack.mjs
```

Passing this pack does not establish an implementation, producer authority,
observation admission, real-data permission, or export approval. The producer
implementation gate and representative exporter remain `BLOCKED`.
