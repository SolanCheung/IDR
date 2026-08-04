#!/usr/bin/env bash
set -euo pipefail

package_root="${1:?usage: check_audit_package_hygiene.sh <package-root>}"

symlink_count="$(find "${package_root}" -type l | wc -l | tr -d ' ')"
if [[ "${symlink_count}" != "0" ]]; then
    printf 'Package contains symbolic links.\n' >&2
    exit 1
fi

forbidden_paths="$(
    find "${package_root}" \
        \( -type d \( \
            -name .aegis -o -name .git -o -name target -o -name node_modules \
            -o -name __pycache__ -o -name .vinext -o -name .next \
            -o -name .wrangler -o -name dist -o -name coverage \
        \) \) -o \
        \( -type f \( \
            -name .env -o -name .env.local -o -name .dev.vars \
            -o -iname 'credentials.json' -o -iname '*.pem' -o -iname '*.key' \
            -o -iname '*.p12' -o -iname '*.pfx' -o -name '*.pyc' -o -name .DS_Store \
        \) \) -print
)"
if [[ -n "${forbidden_paths}" ]]; then
    printf 'Forbidden package paths:\n%s\n' "${forbidden_paths}" >&2
    exit 1
fi

node "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/scan_package_secrets.mjs" \
    "${package_root}"
printf 'PACKAGE_HYGIENE=PASS\n'
