# IDR V1.3 Round 14 audit reproducibility response

Date: 2026-07-31

Final baseline designation: 2026-08-02

Status:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
```

## Frozen scope

Round 14 closes only `P0-AUDIT-REPRO`. It does not add Evidence Qualification,
Admitted Evidence Snapshot, bitemporal Human Model retrieval, vector search,
Setoka integration or any other V1.4 architecture delta.

## Confirmed Round 13 defect

The Round 13 builder copied the host checkout named `Aegis Life` into the
archive as `Aegis-Life`, while the packaged validation script continued to
resolve only `../Aegis Life`. The final ZIP therefore failed before executing
its tests. This did not invalidate the Runtime source fix, but it invalidated
the package's claim of independent replayability.

## Round 14 remediation

1. `tools/resolve_aegis_root.sh` is the only Aegis resolver used by the builder
   and validator. Resolution order is an explicit `IDR_AEGIS_ROOT`, packaged
   `../Aegis-Life`, then source-checkout `../Aegis Life`.
2. The resolver fails closed and verifies `Cargo.toml`, `Cargo.lock` and the
   IDR adapter manifest before returning a canonical path.
3. `tools/run_round14_validation.sh` contains no fixed checkout-only Aegis
   path and verifies the immutable Round 11, Round 12 and Round 13 evidence
   manifests.
4. The builder writes a one-line portable outer sidecar containing only the
   ZIP basename. `tools/validate_round14_archive.sh` rejects non-canonical or
   path-bearing sidecars, authenticates the ZIP, rejects
   unsafe/duplicate/symlink entries, extracts into a new empty temporary
   directory, verifies every package SHA256SUMS entry and invokes the packaged
   Round 14 validation script with the packaged `Aegis-Life`.
5. The final ZIP is built first and then validated by that clean-room entry.
   The resulting clean-room log and its SHA-256 are external evidence bound to
   the already immutable ZIP; the ZIP is not rebuilt after validation.

## External prerequisites

The unique validation entry requires Rust/Cargo, Node/npm, Python,
PostgreSQL 16 with a disposable local test database, `psql`, `openssl`,
`zipinfo`, `unzip`, `rg`, cached offline Cargo/npm dependencies and permission
to create/drop isolated PostgreSQL schemas and roles. Missing prerequisites
fail before a PASS marker and are environment limitations, not silently
skipped tests.

## Claims deliberately not made

Provider credential incident closure, KMS/HSM lifecycle, WORM checkpoint
controls, protected Trust Root distribution, provider-signed permits and the
remaining production release gates are still open. Round 14 is the frozen
final V1.3 audit baseline, not Production authorization.
