#!/usr/bin/env bash
set -euo pipefail

idr_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
aegis_root="$(cd "${idr_root}/../Aegis Life" && pwd)"
round11_evidence="${idr_root}/docs/audit/reviews/2026-07-30-round11-independent"

run() {
    printf '\n=== %s ===\n' "$1"
    shift
    "$@"
}

run "tool versions" bash -c \
    'rustc --version; cargo --version; node --version; npm --version; python3 --version; psql --version'

printf '\n=== immutable Round 11 independent evidence ===\n'
(
    cd "${round11_evidence}"
    shasum -a 256 -c SHA256SUMS
)
printf 'ROUND11_EVIDENCE_ENTRIES=PASS\n'

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
run "strict Clippy migration-only" cargo clippy \
    --manifest-path "${idr_root}/Cargo.toml" -p idr-store \
    --no-default-features --features migration --offline -- -D warnings
run "strict Clippy separate Production migrator" cargo clippy \
    --manifest-path "${idr_root}/tools/idr-production-migrator/Cargo.toml" \
    --locked --offline -- -D warnings
run "production-only compile" cargo check \
    --manifest-path "${idr_root}/Cargo.toml" -p idr-store \
    --no-default-features --features production --offline
run "separate Production migrator compile" cargo check \
    --manifest-path "${idr_root}/tools/idr-production-migrator/Cargo.toml" \
    --locked --offline

printf '\n=== separate Production migrator and role boundary ===\n'
migration_database_url="${IDR_TEST_DATABASE_URL:-postgresql:///postgres}"
migration_schema="idr_production_round12_validation_$$"
migration_runtime_role=""
migration_auditor_role=""
migration_owner_role=""
migration_old_runtime_role=""
migration_old_auditor_role=""
migration_key_file="$(mktemp "${TMPDIR:-/tmp}/idr-round12-attestation-key.XXXXXX")"
chmod 600 "${migration_key_file}"
openssl rand -hex 32 >"${migration_key_file}"
cleanup_migration_fixture() {
    psql "${migration_database_url}" -v ON_ERROR_STOP=1 -q \
        -c "DROP SCHEMA IF EXISTS ${migration_schema} CASCADE" >/dev/null 2>&1 || true
    if [[ -n "${migration_runtime_role}" ]]; then
        psql "${migration_database_url}" -v ON_ERROR_STOP=1 -q \
            -c "DROP ROLE IF EXISTS ${migration_runtime_role}" >/dev/null 2>&1 || true
    fi
    if [[ -n "${migration_auditor_role}" ]]; then
        psql "${migration_database_url}" -v ON_ERROR_STOP=1 -q \
            -c "DROP ROLE IF EXISTS ${migration_auditor_role}" >/dev/null 2>&1 || true
    fi
    if [[ -n "${migration_owner_role}" ]]; then
        psql "${migration_database_url}" -v ON_ERROR_STOP=1 -q \
            -c "DROP OWNED BY ${migration_owner_role}" >/dev/null 2>&1 || true
        psql "${migration_database_url}" -v ON_ERROR_STOP=1 -q \
            -c "DROP ROLE IF EXISTS ${migration_owner_role}" >/dev/null 2>&1 || true
    fi
    if [[ -n "${migration_old_runtime_role}" ]]; then
        psql "${migration_database_url}" -v ON_ERROR_STOP=1 -q \
            -c "DROP ROLE IF EXISTS ${migration_old_runtime_role}" >/dev/null 2>&1 || true
    fi
    if [[ -n "${migration_old_auditor_role}" ]]; then
        psql "${migration_database_url}" -v ON_ERROR_STOP=1 -q \
            -c "DROP ROLE IF EXISTS ${migration_old_auditor_role}" >/dev/null 2>&1 || true
    fi
    find "${migration_key_file}" -delete 2>/dev/null || true
}
trap cleanup_migration_fixture EXIT
psql "${migration_database_url}" -v ON_ERROR_STOP=1 -q \
    -c "CREATE SCHEMA ${migration_schema}"
IDR_MIGRATOR_DATABASE_URL="${migration_database_url}" \
IDR_ORCHESTRATOR_ATTESTATION_KEY_FILE="${migration_key_file}" cargo run --quiet \
    --manifest-path "${idr_root}/tools/idr-production-migrator/Cargo.toml" \
    --locked --offline -- "${migration_schema}"
role_digest="$(
    psql "${migration_database_url}" -Atq \
        -c "SELECT substr(encode(sha256(convert_to('${migration_schema}','UTF8')),'hex'),1,32)"
)"
migration_runtime_role="$(
    printf 'idr_runtime_%s' "${role_digest}"
)"
migration_auditor_role="$(
    printf 'idr_auditor_%s' "${role_digest}"
)"
migration_owner_role="idr_owner_${role_digest}"
migration_old_runtime_role="$(
    psql "${migration_database_url}" -Atq \
        -c "SELECT 'idr_runtime_' || substr(md5('${migration_schema}'),1,16)"
)"
migration_old_auditor_role="$(
    psql "${migration_database_url}" -Atq \
        -c "SELECT 'idr_auditor_' || substr(md5('${migration_schema}'),1,16)"
)"
migration_boundary="$(
    psql "${migration_database_url}" -Atq -F '|' -c "
        SET search_path TO ${migration_schema};
        SELECT
            (SELECT count(*) FROM _sqlx_migrations),
            (SELECT max(version) FROM _sqlx_migrations),
            NOT (SELECT rolcanlogin FROM pg_roles WHERE rolname = '${migration_runtime_role}'),
            (SELECT count(*) = 0
               FROM pg_class relation
               JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
              WHERE namespace.nspname = current_schema()
                AND relation.relkind IN ('r','p')
                AND (
                    has_table_privilege('${migration_runtime_role}', relation.oid, 'INSERT')
                    OR has_table_privilege('${migration_runtime_role}', relation.oid, 'UPDATE')
                    OR has_table_privilege('${migration_runtime_role}', relation.oid, 'DELETE')
                    OR has_table_privilege('${migration_runtime_role}', relation.oid, 'TRUNCATE')
                )),
            (SELECT count(*) = 22
               FROM pg_class relation
               JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
              WHERE namespace.nspname = current_schema()
                AND relation.relkind = 'v'
                AND relation.relname LIKE 'idr\\_%' ESCAPE '\\'
                AND has_table_privilege(
                    '${migration_runtime_role}', relation.oid, 'SELECT,INSERT,UPDATE'
                )),
            NOT has_table_privilege(
                '${migration_runtime_role}',
                '${migration_schema}.idr_orchestrator_attestation_secrets_v7',
                'SELECT'
            ),
            has_function_privilege(
                '${migration_runtime_role}',
                '${migration_schema}.idr_open_attested_transition_v7(text,text,text,uuid,uuid,uuid,text,bigint,bigint,text,text,bigint,integer,uuid,bigint,text)',
                'EXECUTE'
            ),
            has_table_privilege(
                '${migration_auditor_role}',
                '${migration_schema}.idr_operation_idempotency_fences',
                'SELECT'
            ),
            NOT has_table_privilege(
                '${migration_auditor_role}',
                '${migration_schema}.idr_operation_idempotency_fences',
                'INSERT'
            );
    " | tail -1
)"
if [[ "${migration_boundary}" != "7|7|t|t|t|t|t|t|t" ]]; then
    printf 'Unexpected Production migration/role boundary: %s\n' "${migration_boundary}" >&2
    exit 1
fi
printf 'PRODUCTION_MIGRATOR_SCHEMA_VERSION=7\n'
printf 'PRODUCTION_RUNTIME_BASE_TABLE_DML=0\n'
printf 'PRODUCTION_RUNTIME_ATTESTED_VIEW_COUNT=22\n'
printf 'PRODUCTION_RUNTIME_KEY_READ=DENIED\n'
printf 'PRODUCTION_AUDITOR_ROLE_READ_ONLY=PASS\n'
cleanup_migration_fixture
trap - EXIT

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

expected_feature_compile_failure() {
    local label="$1"
    local package="$2"
    local features="$3"
    local expected="$4"
    printf '\n=== %s ===\n' "${label}"
    set +e
    local output
    output="$(
        cargo check --manifest-path "${idr_root}/Cargo.toml" \
            -p "${package}" --no-default-features --features "${features}" \
            --offline 2>&1
    )"
    local status=$?
    set -e
    printf '%s\n' "${output}"
    if [[ ${status} -eq 0 ]] || ! grep -q "${expected}" <<<"${output}"; then
        printf 'Expected feature compile failure was not observed: %s\n' "${label}" >&2
        exit 1
    fi
    printf 'EXPECTED_FAILURE_STATUS=%s\n' "${status}"
}

expected_feature_compile_failure \
    "production + migration feature gate" \
    "idr-store" \
    "production,migration" \
    "production runtime and migration authority are mutually exclusive"
expected_feature_compile_failure \
    "shadow-mode + migration feature gate" \
    "idr-runtime" \
    "shadow-mode,migration" \
    "Shadow runtime and migration authority are mutually exclusive"

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

printf '\n=== Round 12 targeted static checks ===\n'
for marker in \
    'require_historical_evidence_record' \
    'IDR_RECEIPT_OUTCOME_OBSERVATION_WINDOW_SECONDS_V1' \
    'IDR_OUTCOME_HUMAN_MODEL_WINDOW_SECONDS_V1' \
    'execution_cancelled_invalid_authority' \
    'execution_reconciliation_required_invalid_authority'; do
    rg -Fq "${marker}" "${idr_root}/crates/idr-runtime/src/trust_chain.rs"
done
for marker in \
    'PostgresIdrMigratorV1' \
    'verify_production_runtime_role' \
    'open_orchestrator_attestation' \
    'verify_all_orchestrator_attestations' \
    'governance_proof_revocation' \
    'idr_operation_idempotency_fences' \
    'idempotency_fence_digest_v1'; do
    rg -Fq "${marker}" "${idr_root}/crates/idr-runtime/src/postgres_authority.rs"
done
for marker in \
    'idr_orchestrator_attestation_secrets_v7' \
    'idr_transition_attestations_v7' \
    'idr_open_attested_transition_v7' \
    'idr_attested_authority_view_bridge_v7' \
    'sha256(convert_to(current_schema()' \
    'REVOKE ALL ON _sqlx_migrations'; do
    rg -Fq "${marker}" \
        "${idr_root}/crates/idr-store/migrations/0007_round12_attested_authority_boundary.sql"
done
for attack in \
    'revoked_key_is_rechecked_at_transaction_consumption_time_on_idempotent_replay' \
    'historical_evidence_windows_are_explicit_bounded_and_fail_closed' \
    'invalid_action_delivery_persists_cancellation_instead_of_rolling_back' \
    'expired_authority_after_dispatch_persists_reconciliation_state' \
    'raw_runtime_login_cannot_forge_authority_rows' \
    'stolen_runtime_login_has_zero_base_table_dml' \
    'pre_restart_duplicate_reservation_is_blocked_by_append_only_fence' \
    'action_admission_proof_revoked_cancelled'; do
    rg -Fq "${attack}" "${idr_root}/crates"
done
printf 'ROUND12_STATIC_CLOSURE_MARKERS=PASS\n'

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
printf 'API_KEY=%s%s\n' 'sk-' 'Round12SyntheticCredentialOnly000000' \
    >"${scanner_fixture}/synthetic.env"
set +e
scanner_output="$(
    node "${idr_root}/tools/scan_package_secrets.mjs" "${scanner_fixture}" 2>&1
)"
scanner_status=$?
set -e
if [[ ${scanner_status} -eq 0 ]] ||
   ! grep -q '"pattern": "openai_like"' <<<"${scanner_output}" ||
   grep -q 'Round12SyntheticCredentialOnly' <<<"${scanner_output}"; then
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

printf '\nALL_ROUND12_VALIDATION_STEPS=PASS\n'
