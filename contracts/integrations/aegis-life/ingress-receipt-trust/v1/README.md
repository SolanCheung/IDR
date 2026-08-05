# IDR × Aegis Life Ingress Receipt Trust Conformance Pack V1

Status: **Specification Only / Receipt Source Not Implemented**

This pack locks the source owner, UUID namespaces, run/turn provenance, actor
pseudonymization, exact-byte content digest, correlation semantics, monotonic
logical clock, source-binding requirement, and fail-closed implementation gate
for a future `IngressReceiptV1` source.

Contents:

- `idr-aegis-ingress-receipt-source-contract-v1.schema.json`: strict neutral
  source-contract schema;
- `fixtures/valid/synthetic-ingress-receipt-source-contract-v1.json`:
  synthetic golden contract with the source `NOT_IMPLEMENTED`;
- `manifest.json`: schema and golden digests plus negative mutations;
- `verify-pack.mjs`: dependency-free offline verifier.

Run:

```bash
node contracts/integrations/aegis-life/ingress-receipt-trust/v1/verify-pack.mjs
```

Passing this pack does not establish a source owner, receipt implementation,
trusted clock, observation admission, real-data permission, or export approval.
