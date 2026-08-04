#!/usr/bin/env bash
set -euo pipefail

idr_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
aegis_root="$(cd "${idr_root}/../Aegis Life" && pwd)"
output_zip="${1:-/Users/solan/Downloads/IDR-V1.3-trust-chain-closure-round13-2026-07-30.zip}"
output_sha="${output_zip}.sha256"

if [[ -e "${output_zip}" || -e "${output_sha}" ]]; then
    printf 'Refusing to overwrite an existing package: %s\n' "${output_zip}" >&2
    exit 1
fi

stage_parent="$(mktemp -d "${TMPDIR:-/tmp}/idr-round13-package.XXXXXX")"
package_name="IDR-V1.3-trust-chain-closure-round13-2026-07-30"
package_root="${stage_parent}/${package_name}"
mkdir -p "${package_root}/IDR" "${package_root}/Aegis-Life"

cleanup() {
    find "${stage_parent}" -depth -delete 2>/dev/null || true
}
trap cleanup EXIT

copy_file() {
    local source="$1"
    local destination="$2"
    if [[ ! -f "${source}" ]]; then
        printf 'Required package file is absent: %s\n' "${source}" >&2
        exit 1
    fi
    cp -p "${source}" "${destination}"
}

copy_tree() {
    local source="$1"
    local destination="$2"
    rsync -a \
        --exclude='.git/' \
        --exclude='.aegis/' \
        --exclude='target/' \
        --exclude='node_modules/' \
        --exclude='__pycache__/' \
        --exclude='.vinext/' \
        --exclude='.next/' \
        --exclude='.wrangler/' \
        --exclude='dist/' \
        --exclude='coverage/' \
        --exclude='.DS_Store' \
        --exclude='.env' \
        --exclude='.env.local' \
        --exclude='.dev.vars' \
        --exclude='credentials.json' \
        --exclude='*.pem' \
        --exclude='*.key' \
        --exclude='*.p12' \
        --exclude='*.pfx' \
        --exclude='*.pyc' \
        "${source}/" "${destination}/"
}

for file in \
    .gitignore Cargo.lock Cargo.toml \
    IDR-V1.3-Trust-Chain-Closure-Master-Spec.md \
    IDR-V1.3-round5-reaudit-report-2026-07-29.md README.md; do
    copy_file "${idr_root}/${file}" "${package_root}/IDR/${file}"
done
for directory in \
    contracts crates docs integrations packages research source-design tools; do
    mkdir -p "${package_root}/IDR/${directory}"
    copy_tree "${idr_root}/${directory}" "${package_root}/IDR/${directory}"
done

for file in .gitignore Cargo.lock Cargo.toml ARCHITECTURE.md MIGRATION.md README.md; do
    copy_file "${aegis_root}/${file}" "${package_root}/Aegis-Life/${file}"
done
for directory in \
    .github adapters apps configs cores crates deploy docs scripts src; do
    if [[ -d "${aegis_root}/${directory}" ]]; then
        mkdir -p "${package_root}/Aegis-Life/${directory}"
        copy_tree "${aegis_root}/${directory}" "${package_root}/Aegis-Life/${directory}"
    fi
done

copy_file \
    "${idr_root}/docs/audit/IDR_V1_3_ROUND13_PACKAGE_README.md" \
    "${package_root}/README.md"
copy_file \
    "${idr_root}/docs/audit/IDR_V1_3_ROUND13_AUDIT_MANIFEST.md" \
    "${package_root}/AUDIT-MANIFEST.md"
copy_file \
    "${idr_root}/docs/trust-chain-closure/ROUND13-REAUDIT-RESPONSE.md" \
    "${package_root}/ROUND13-REAUDIT-RESPONSE.md"
copy_file \
    "${idr_root}/docs/trust-chain-closure/VALIDATION-SUMMARY.md" \
    "${package_root}/VALIDATION-SUMMARY.md"

"${idr_root}/tools/check_audit_package_hygiene.sh" "${package_root}"

(
    cd "${package_root}"
    : >SHA256SUMS
    : >FILE-INVENTORY.txt
    find . -type f -print | LC_ALL=C sort >FILE-INVENTORY.txt
    find . -type f ! -path './SHA256SUMS' -print0 |
        sort -z |
        xargs -0 shasum -a 256 >SHA256SUMS
)

(
    cd "${stage_parent}"
    zip -q -X -r "${output_zip}" "${package_name}"
)
shasum -a 256 "${output_zip}" >"${output_sha}"

printf 'PACKAGE=%s\n' "${output_zip}"
printf 'PACKAGE_SHA256=%s\n' "$(awk '{print $1}' "${output_sha}")"
printf 'PACKAGE_FILES=%s\n' \
    "$(find "${package_root}" -type f | wc -l | tr -d ' ')"
