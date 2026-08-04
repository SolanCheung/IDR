# Expected compile failure

This downstream crate deliberately enables `production + shadow-mode`.
Validation passes only when the IDR trust-domain feature guard rejects it.
