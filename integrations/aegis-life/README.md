# Aegis Life reference integration

Aegis Life consumes IDR through its `crates/idr-aegis-adapter` crate.

The adapter is deliberately thin:

- it maps Aegis production references and authority data into
  `HostAuthorityCandidateV1`; a candidate is not an IDR Authority Context or
  grant;
- it exposes a narrow `assess_shadow` façade and does not re-export raw
  protocol, runtime, or store crates;
- it does not duplicate IDR guards or grant authority;
- it allows Aegis Life to act as a shadow-mode reference host.
- it does not depend on `idr-store`.

The adapter now has explicit Cargo features:

- default `shadow-mode` enables only the legacy shadow assessment surface;
- `aegis-production-adapter` is compile-blocked until the IDR production
  release latch and independent audit gates pass.

The adapter is not a production execution admission boundary. IDR remains
blocked for production trust-root use until the open requirements in
`docs/audit/IDR_V1_3_REMEDIATION.md` are closed and independently re-audited.
