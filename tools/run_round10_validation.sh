#!/usr/bin/env bash
set -euo pipefail

idr_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
aegis_root="$(cd "${idr_root}/../Aegis Life" && pwd)"
round9_evidence="${idr_root}/docs/audit/reviews/2026-07-30-round9-independent"

run() {
    printf '\n=== %s ===\n' "$1"
    shift
    "$@"
}

run "tool versions" bash -c \
    'rustc --version; cargo --version; node --version; npm --version; python3 --version; psql --version'

printf '\n=== immutable Round 9 independent evidence ===\n'
(
    cd "${round9_evidence}"
    awk '$2 != "./SHA256SUMS" && $2 != "SHA256SUMS"' SHA256SUMS |
        shasum -a 256 -c -
)
printf 'ROUND9_EVIDENCE_NON_SELF_ENTRIES=PASS\n'
printf 'ROUND9_EVIDENCE_SELF_REFERENTIAL_MANIFEST=RETAINED_AS_AUDITOR_ARTIFACT\n'

run "rustfmt" cargo fmt --manifest-path "${idr_root}/Cargo.toml" --all -- --check
run "Rust default workspace tests" cargo test \
    --manifest-path "${idr_root}/Cargo.toml" --workspace --offline
run "Rust Shadow PostgreSQL attack tests" env \
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

expected_compile_failure() {
    local label="$1"
    local manifest="$2"
    local expected="$3"
    printf '\n=== %s ===\n' "${label}"
    set +e
    local output
    output="$(cargo check --manifest-path "${manifest}" --locked --offline 2>&1)"
    local status=$?
    set -e
    printf '%s\n' "${output}"
    if [[ ${status} -eq 0 ]] || ! grep -q "${expected}" <<<"${output}"; then
        printf 'Expected compile failure was not observed: %s\n' "${label}" >&2
        exit 1
    fi
    printf 'EXPECTED_FAILURE_STATUS=%s\n' "${status}"
}

expected_compile_failure \
    "production + test-support feature gate" \
    "${idr_root}/tools/production-test-support-compile-fail/Cargo.toml" \
    "production and test-support are mutually exclusive"
expected_compile_failure \
    "production + shadow-mode feature gate" \
    "${idr_root}/tools/production-shadow-compile-fail/Cargo.toml" \
    "production and shadow-mode are mutually exclusive"

printf '\n=== production authority API compile gate ===\n'
set +e
api_output="$(
    cargo check --manifest-path \
        "${idr_root}/tools/production-api-compile-fail/Cargo.toml" \
        --locked --offline 2>&1
)"
api_status=$?
set -e
printf '%s\n' "${api_output}"
for expected in \
    'no method named `pool_for_test`' \
    'no method named `inspect_for_test`' \
    'no method named `verify_external_anchor_for_test`'; do
    if ! grep -q "${expected}" <<<"${api_output}"; then
        printf 'Missing expected production API denial: %s\n' "${expected}" >&2
        exit 1
    fi
done
if [[ ${api_status} -eq 0 ]]; then
    printf 'Production bypass fixture unexpectedly compiled.\n' >&2
    exit 1
fi
printf 'EXPECTED_FAILURE_STATUS=%s\n' "${api_status}"

printf '\n=== Round 10 targeted static checks ===\n'
for marker in \
    'require_record_at' \
    'require_exact_current_record_at' \
    'ACTION_ADMISSION_PROOF_KINDS_V1' \
    'verified_action_admission_proof_bindings'; do
    rg -q "${marker}" "${idr_root}/crates/idr-runtime/src/trust_chain.rs"
done
for marker in \
    'reverify_bound_action_admission_proofs' \
    'verify_current_execution_projection' \
    'execution_state_root'; do
    rg -q "${marker}" "${idr_root}/crates/idr-runtime/src/postgres_authority.rs"
done
for marker in \
    'idr_execution_reservation_update_guard_v5' \
    'idr_execution_attempt_update_guard_v5' \
    'idr_audit_checkpoint_update_guard_v5'; do
    rg -q "${marker}" \
        "${idr_root}/crates/idr-store/migrations/0005_round10_execution_integrity.sql"
done
for attack in \
    'expired_context_rejects_intent' \
    'expired_intent_rejects_decision' \
    'expired_decision_rejects_action' \
    'expired_turn_rejects_action' \
    'expired_response_cannot_send' \
    'revoked_policy_after_admission_blocks_reserve' \
    'execution_identity_update_is_rejected' \
    'operation_idempotency_slot_cannot_be_released_by_sql_update' \
    'execution_index_tamper_detected_on_startup'; do
    rg -q "${attack}" "${idr_root}/crates"
done
printf 'ROUND10_STATIC_CLOSURE_MARKERS=PASS\n'

if [[ -e "${aegis_root}/apps/aegis-web/.aegis/credentials.json" ||
      -e "${aegis_root}/apps/aegis-web/.dev.vars" ]]; then
    printf 'Known leaked credential source path still exists.\n' >&2
    exit 1
fi
printf 'KNOWN_LEAKED_SOURCE_PATHS=ABSENT\n'

printf '\n=== package-secret scanner self-test ===\n'
scanner_fixture="$(mktemp -d "${TMPDIR:-/tmp}/idr-secret-scanner.XXXXXX")"
cleanup_fixture() {
    find "${scanner_fixture}" -depth -delete 2>/dev/null || true
}
trap cleanup_fixture EXIT
printf 'API_KEY=%s%s\n' 'sk-' 'Round10SyntheticCredentialOnly000000' \
    >"${scanner_fixture}/synthetic.env"
set +e
scanner_output="$(
    node "${idr_root}/tools/scan_package_secrets.mjs" "${scanner_fixture}" 2>&1
)"
scanner_status=$?
set -e
if [[ ${scanner_status} -eq 0 ]] ||
   ! grep -q '"pattern": "openai_like"' <<<"${scanner_output}" ||
   grep -q 'Round10SyntheticCredentialOnly' <<<"${scanner_output}"; then
    printf 'Secret scanner self-test failed or exposed raw secret text.\n' >&2
    exit 1
fi
printf 'SECRET_SCANNER_DETECTION=PASS\n'
printf 'SECRET_SCANNER_RAW_VALUE_DISCLOSURE=0\n'
cleanup_fixture
trap - EXIT

scanner_empty_fixture="$(mktemp -d "${TMPDIR:-/tmp}/idr-empty-secret-scanner.XXXXXX")"
cleanup_empty_fixture() {
    find "${scanner_empty_fixture}" -depth -delete 2>/dev/null || true
}
trap cleanup_empty_fixture EXIT
printf 'API_KEY=\nMODEL=example-model\nENDPOINT=http://127.0.0.1:3010/v1\n' \
    >"${scanner_empty_fixture}/example.env"
node "${idr_root}/tools/scan_package_secrets.mjs" "${scanner_empty_fixture}"
printf 'SECRET_SCANNER_EMPTY_ASSIGNMENT_CROSS_LINE=PASS\n'
cleanup_empty_fixture
trap - EXIT

run "IDR source secret scan" node \
    "${idr_root}/tools/scan_package_secrets.mjs" "${idr_root}"

run "TypeScript clean offline install" npm --prefix \
    "${idr_root}/packages/interaction-client" ci --offline
run "TypeScript tests" npm --prefix "${idr_root}/packages/interaction-client" test
run "TypeScript typecheck" npm --prefix \
    "${idr_root}/packages/interaction-client" run typecheck
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
aegis_output="$(
    cargo check --manifest-path "${aegis_root}/Cargo.toml" \
        -p idr-aegis-adapter --no-default-features \
        --features aegis-production-adapter --offline 2>&1
)"
aegis_status=$?
set -e
printf '%s\n' "${aegis_output}"
if [[ ${aegis_status} -eq 0 ]] ||
   ! grep -q "aegis-production-adapter remains BLOCKED" <<<"${aegis_output}"; then
    printf 'Expected Aegis production compile gate was not observed.\n' >&2
    exit 1
fi
printf 'EXPECTED_FAILURE_STATUS=%s\n' "${aegis_status}"

run "Aegis full workspace" cargo test \
    --manifest-path "${aegis_root}/Cargo.toml" --workspace --offline
run "dependency direction" cargo tree \
    --manifest-path "${aegis_root}/Cargo.toml" -p idr-aegis-adapter --offline

printf '\nALL_ROUND10_VALIDATION_STEPS=PASS\n'
