# Expected compile failure

This downstream crate deliberately enables `production + test-support`.
Validation passes only when the IDR feature-unification guard rejects it.
