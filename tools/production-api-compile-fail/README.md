# Expected compile failure

This downstream crate enables only `idr-runtime/production` and deliberately
attempts to call the test-only database/inspect helpers. Validation passes only
when `cargo check` fails with all three “method not found” diagnostics.
