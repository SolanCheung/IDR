# IDR × Aegis Life Source Export Approval Trust Pack V1

Status: **Specification Only / Synthetic Control Only**

This dependency-free pack tests exact approval scope, trusted-current-time
validity, revocation binding, and fail-closed request binding. Its only positive
fixture is synthetic and returns `ALLOW_SYNTHETIC_CONTROL_ONLY`.

Run:

```bash
node contracts/integrations/aegis-life/source-export-approval/v1/verify-pack.mjs
```

Passing does not create lawful basis, a production issuer or clock, an Aegis
exporter, source access, or real-data authorization.
