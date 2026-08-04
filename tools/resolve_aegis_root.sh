#!/usr/bin/env bash

# Resolve the explicit Aegis dependency without assuming that a source checkout
# and an extracted audit archive use the same directory spelling.
resolve_aegis_root() {
    local idr_root_input="${1:?IDR root is required}"
    local candidate
    local resolved

    if [[ -n "${IDR_AEGIS_ROOT:-}" ]]; then
        if [[ ! -d "${IDR_AEGIS_ROOT}" ]]; then
            printf 'IDR_AEGIS_ROOT does not name an existing directory\n' >&2
            return 1
        fi
        candidate="${IDR_AEGIS_ROOT}"
    else
        candidate=""
        for resolved in \
            "${idr_root_input}/../Aegis-Life" \
            "${idr_root_input}/../Aegis Life"; do
            if [[ -d "${resolved}" ]]; then
                candidate="${resolved}"
                break
            fi
        done
        if [[ -z "${candidate}" ]]; then
            printf 'Aegis root not found beside the IDR root\n' >&2
            return 1
        fi
    fi

    resolved="$(cd "${candidate}" && pwd -P)"
    for required in \
        Cargo.toml \
        Cargo.lock \
        crates/idr-aegis-adapter/Cargo.toml; do
        if [[ ! -f "${resolved}/${required}" ]]; then
            printf 'Aegis root is missing required file: %s\n' "${required}" >&2
            return 1
        fi
    done
    printf '%s\n' "${resolved}"
}

