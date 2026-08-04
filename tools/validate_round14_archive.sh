#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    printf 'usage: validate_round14_archive.sh <round14.zip>\n' >&2
    exit 2
fi

archive_input="$1"
if [[ ! -f "${archive_input}" ]]; then
    printf 'Round 14 archive does not exist\n' >&2
    exit 2
fi
archive_dir="$(cd "$(dirname "${archive_input}")" && pwd -P)"
archive_path="${archive_dir}/$(basename "${archive_input}")"
sidecar_path="${archive_path}.sha256"
if [[ ! -f "${sidecar_path}" ]]; then
    printf 'Round 14 outer SHA-256 sidecar is absent\n' >&2
    exit 2
fi
sidecar_line_count="$(wc -l <"${sidecar_path}" | tr -d ' ')"
IFS=' ' read -r sidecar_digest sidecar_name sidecar_extra <"${sidecar_path}"
if [[ "${sidecar_line_count}" != "1" ||
      ! "${sidecar_digest}" =~ ^[0-9a-f]{64}$ ||
      "${sidecar_name}" != "$(basename "${archive_path}")" ||
      -n "${sidecar_extra:-}" ]]; then
    printf 'Round 14 outer SHA-256 sidecar is not portable or canonical\n' >&2
    exit 1
fi

extract_parent="$(mktemp -d "${TMPDIR:-/tmp}/idr-round14-clean-room.XXXXXX")"
entry_list="$(mktemp "${TMPDIR:-/tmp}/idr-round14-entries.XXXXXX")"
cleanup() {
    find "${extract_parent}" -depth -delete 2>/dev/null || true
    find "${entry_list}" -delete 2>/dev/null || true
}
trap cleanup EXIT

(cd "${archive_dir}" && shasum -a 256 -c "$(basename "${sidecar_path}")")
unzip -t "${archive_path}"
zipinfo -1 "${archive_path}" >"${entry_list}"
if awk '/^\// || /(^|\/)\.\.($|\/)/ { found=1 } END { exit found ? 0 : 1 }' \
    "${entry_list}"; then
    printf 'Round 14 archive contains an unsafe path\n' >&2
    exit 1
fi
if [[ "$(sort "${entry_list}" | uniq -d | wc -l | tr -d ' ')" != "0" ]]; then
    printf 'Round 14 archive contains duplicate paths\n' >&2
    exit 1
fi
if [[ "$(zipinfo -l "${archive_path}" |
    awk 'substr($1,1,1)=="l"{count++} END{print count+0}')" != "0" ]]; then
    printf 'Round 14 archive contains a symbolic link\n' >&2
    exit 1
fi

unzip -q "${archive_path}" -d "${extract_parent}"
root_count="$(
    find "${extract_parent}" -mindepth 1 -maxdepth 1 -type d -print |
        wc -l |
        tr -d ' '
)"
if [[ "${root_count}" != "1" ]]; then
    printf 'Round 14 archive must contain exactly one top-level directory\n' >&2
    exit 1
fi
package_root="$(
    find "${extract_parent}" -mindepth 1 -maxdepth 1 -type d -print -quit
)"
if [[ ! -d "${package_root}/IDR" ||
      ! -d "${package_root}/Aegis-Life" ||
      ! -x "${package_root}/IDR/tools/run_round14_validation.sh" ]]; then
    printf 'Round 14 extracted package layout is incomplete\n' >&2
    exit 1
fi

(cd "${package_root}" && shasum -a 256 -c SHA256SUMS)
printf 'ROUND14_CLEAN_ROOM_ARCHIVE_INTEGRITY=PASS\n'
printf 'ROUND14_CLEAN_ROOM_LAYOUT=PASS\n'

IDR_AEGIS_ROOT="${package_root}/Aegis-Life" \
    "${package_root}/IDR/tools/run_round14_validation.sh"

printf '\nALL_ROUND14_CLEAN_ROOM_STEPS=PASS\n'
