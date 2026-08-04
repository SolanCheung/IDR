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
    "${idr_root}/docs/audit/reviews/2026-07-30-round7-independent/IDR-V1.3-round7-independent-reaudit-report-2026-07-30.md" \
    "${idr_root}/docs/audit/reviews/2026-07-30-round7-independent/idr-v13-round7-independent-test-output.txt" \
    "${idr_root}/docs/audit/reviews/2026-07-30-round7-independent/idr-v13-round7-static-audit-checks.txt"
run "rustfmt" cargo fmt --manifest-path "${idr_root}/Cargo.toml" --all -- --check
run "Rust default workspace tests" cargo test \
    --manifest-path "${idr_root}/Cargo.toml" --workspace --offline
run "Rust Shadow PostgreSQL integration tests" env \
    IDR_TEST_DATABASE_URL=postgresql:///postgres cargo test \
    --manifest-path "${idr_root}/Cargo.toml" -p idr-store \
    --no-default-features --features shadow-mode \
    --test postgres_trust_chain --test postgres_vertical_slice --offline
run "Rust Production release-gate unit tests" cargo test \
    --manifest-path "${idr_root}/Cargo.toml" -p idr-runtime \
    --no-default-features --features production --lib --offline
run "strict Clippy default" cargo clippy \
    --manifest-path "${idr_root}/Cargo.toml" --workspace --all-targets \
    --offline -- -D warnings
run "strict Clippy Shadow PostgreSQL" cargo clippy \
    --manifest-path "${idr_root}/Cargo.toml" -p idr-store \
    --no-default-features --features shadow-mode --lib \
    --test postgres_trust_chain --test postgres_vertical_slice \
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

printf '\n=== production + test-support feature-unification gate ===\n'
set +e
production_test_output="$(
    cargo check --manifest-path \
        "${idr_root}/tools/production-test-support-compile-fail/Cargo.toml" \
        --locked --offline 2>&1
)"
production_test_status=$?
set -e
printf '%s\n' "${production_test_output}"
if [[ ${production_test_status} -eq 0 ]] ||
   ! grep -q "production and test-support are mutually exclusive" <<<"${production_test_output}"; then
    printf 'Expected production + test-support compile failure was not observed.\n' >&2
    exit 1
fi
printf 'EXPECTED_FAILURE_STATUS=%s\n' "${production_test_status}"

printf '\n=== production + shadow-mode trust-domain feature gate ===\n'
set +e
production_shadow_output="$(
    cargo check --manifest-path \
        "${idr_root}/tools/production-shadow-compile-fail/Cargo.toml" \
        --locked --offline 2>&1
)"
production_shadow_status=$?
set -e
printf '%s\n' "${production_shadow_output}"
if [[ ${production_shadow_status} -eq 0 ]] ||
   ! grep -q "production and shadow-mode are mutually exclusive" <<<"${production_shadow_output}"; then
    printf 'Expected production + shadow-mode compile failure was not observed.\n' >&2
    exit 1
fi
printf 'EXPECTED_FAILURE_STATUS=%s\n' "${production_shadow_status}"

printf '\n=== Round 8 static authority checks ===\n'
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
if rg -n 'PostgresStartupModeV1|pub async fn connect\\(' \
    "${idr_root}/crates/idr-runtime/src/postgres_authority.rs"; then
    printf 'A caller-selectable Shadow/Production mode remains.\n' >&2
    exit 1
fi
proof_subject_source="$(
    sed -n '/pub fn proof_subject/,/pub fn required_proof_kinds/p' \
        "${idr_root}/crates/idr-runtime/src/trust_chain.rs"
)"
if grep -q 'candidate' <<<"${proof_subject_source}"; then
    printf 'Candidate-only proof binding remains.\n' >&2
    exit 1
fi
printf 'CALLER_SELECTABLE_STARTUP_MODE=0\n'
printf 'CANDIDATE_ONLY_PROOF_SUBJECT=0\n'

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
