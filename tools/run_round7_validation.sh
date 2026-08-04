#!/usr/bin/env bash
set -euo pipefail

idr_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
aegis_root="$(cd "${idr_root}/../Aegis Life" && pwd)"

run() {
    printf '\n=== %s ===\n' "$1"
    shift
    "$@"
}

run "tool versions" bash -c 'rustc --version; cargo --version; node --version; npm --version; python3 --version; psql --version'
run "immutable source evidence" shasum -a 256 \
    "${idr_root}/IDR-V1.3-Trust-Chain-Closure-Master-Spec.md" \
    "${idr_root}/IDR-V1.3-round5-reaudit-report-2026-07-29.md" \
    "${idr_root}/source-design/IDR-V1.3-original-design.txt" \
    "${idr_root}/docs/audit/reviews/2026-07-29-round6-independent/IDR-V1.3-round6-independent-reaudit-report-2026-07-29.md" \
    "${idr_root}/docs/audit/reviews/2026-07-29-round6-independent/idr-v13-round6-independent-test-output.txt" \
    "${idr_root}/docs/audit/reviews/2026-07-29-round6-independent/idr-v13-round6-static-audit-checks.txt"
run "rustfmt" cargo fmt --manifest-path "${idr_root}/Cargo.toml" --all -- --check
run "Rust workspace tests including PostgreSQL" env \
    IDR_TEST_DATABASE_URL=postgresql:///postgres \
    cargo test --manifest-path "${idr_root}/Cargo.toml" --workspace --offline
run "strict Clippy default/shadow" cargo clippy \
    --manifest-path "${idr_root}/Cargo.toml" --workspace --all-targets \
    --offline -- -D warnings
run "strict Clippy production-only" cargo clippy \
    --manifest-path "${idr_root}/Cargo.toml" -p idr-store \
    --no-default-features --features production --offline -- -D warnings
run "production-only compile" cargo check \
    --manifest-path "${idr_root}/Cargo.toml" -p idr-store \
    --no-default-features --features production --offline

printf '\n=== production bypass API compile gate ===\n'
set +e
production_api_output="$(
    cargo check --manifest-path \
        "${idr_root}/tools/production-api-compile-fail/Cargo.toml" \
        --locked --offline 2>&1
)"
production_api_status=$?
set -e
printf '%s\n' "${production_api_output}"
if [[ ${production_api_status} -eq 0 ]] ||
   ! grep -q 'no method named `pool_for_test`' <<<"${production_api_output}" ||
   ! grep -q 'no method named `inspect_for_test`' <<<"${production_api_output}" ||
   ! grep -q 'no method named `verify_external_anchor_for_test`' <<<"${production_api_output}"; then
    printf 'Expected production API compile failures were not observed.\n' >&2
    exit 1
fi
printf 'EXPECTED_FAILURE_STATUS=%s\n' "${production_api_status}"

printf '\n=== Round 7 static authority checks ===\n'
if rg -n 'pub (async )?fn evaluate_authoritative_transition_v1|pub trait IdrTransactionalRepositoryV1|struct IdrOrchestratorV1<' \
    "${idr_root}/crates"; then
    printf 'A public authority bypass symbol remains.\n' >&2
    exit 1
fi
if rg -n 'ExactActionAuthorizationSubmissionV1|buildExactActionAuthorizationSubmission' \
    "${idr_root}/packages/interaction-client/src" \
    "${idr_root}/packages/interaction-client/tests"; then
    printf 'The misleading TypeScript Authorization Submission name remains.\n' >&2
    exit 1
fi
printf 'PUBLIC_AUTHORITY_BYPASS_SYMBOLS=0\n'
printf 'TYPESCRIPT_AUTHORIZATION_SUBMISSION_NAMES=0\n'

printf '\n=== incompatible production + dev-file-store compile gate ===\n'
set +e
incompatible_output="$(
    cargo check --manifest-path "${idr_root}/Cargo.toml" -p idr-store \
        --features production --offline 2>&1
)"
incompatible_status=$?
set -e
printf '%s\n' "${incompatible_output}"
if [[ ${incompatible_status} -eq 0 ]] ||
   ! grep -q "production and dev-file-store are mutually exclusive" <<<"${incompatible_output}"; then
    printf 'Expected compile failure was not observed.\n' >&2
    exit 1
fi
printf 'EXPECTED_FAILURE_STATUS=%s\n' "${incompatible_status}"

run "TypeScript clean offline install" npm --prefix \
    "${idr_root}/packages/interaction-client" ci --offline
run "TypeScript tests" npm --prefix "${idr_root}/packages/interaction-client" test
run "TypeScript typecheck" npm --prefix "${idr_root}/packages/interaction-client" run typecheck
run "Python tests" bash -c \
    "cd '${idr_root}/research/evaluation' && PYTHONPATH=src python3 -m unittest discover -s tests -v"

run "generated contract drift check" bash -c "
    before=\$(shasum -a 256 \
      '${idr_root}/contracts/production/v1/idr-production-v1.schema.json' \
      '${idr_root}/contracts/production/v1/canonical-golden-v1.json' \
      '${idr_root}/packages/interaction-client/src/generated/idr-production-v1.ts' \
      '${idr_root}/research/evaluation/src/idr_eval/generated_production_v1.py')
    cargo run --manifest-path '${idr_root}/Cargo.toml' -p idr-protocol \
      --example generate_production_contracts --offline >/dev/null
    after=\$(shasum -a 256 \
      '${idr_root}/contracts/production/v1/idr-production-v1.schema.json' \
      '${idr_root}/contracts/production/v1/canonical-golden-v1.json' \
      '${idr_root}/packages/interaction-client/src/generated/idr-production-v1.ts' \
      '${idr_root}/research/evaluation/src/idr_eval/generated_production_v1.py')
    test \"\$before\" = \"\$after\"
    printf '%s\n' \"\$after\"
    printf 'GENERATED_DRIFT=0\n'
"

run "Aegis IDR shadow adapter" cargo test \
    --manifest-path "${aegis_root}/Cargo.toml" -p idr-aegis-adapter --offline

printf '\n=== Aegis production adapter compile gate ===\n'
set +e
aegis_gate_output="$(
    cargo check --manifest-path "${aegis_root}/Cargo.toml" \
        -p idr-aegis-adapter --no-default-features \
        --features aegis-production-adapter --offline 2>&1
)"
aegis_gate_status=$?
set -e
printf '%s\n' "${aegis_gate_output}"
if [[ ${aegis_gate_status} -eq 0 ]] ||
   ! grep -q "aegis-production-adapter remains BLOCKED" <<<"${aegis_gate_output}"; then
    printf 'Expected Aegis compile failure was not observed.\n' >&2
    exit 1
fi
printf 'EXPECTED_FAILURE_STATUS=%s\n' "${aegis_gate_status}"

run "Aegis full workspace" cargo test \
    --manifest-path "${aegis_root}/Cargo.toml" --workspace --offline
run "dependency direction" cargo tree \
    --manifest-path "${aegis_root}/Cargo.toml" -p idr-aegis-adapter --offline

printf '\nALL_VALIDATION_STEPS=PASS\n'
