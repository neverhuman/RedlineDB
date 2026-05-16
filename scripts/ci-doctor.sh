#!/usr/bin/env bash
# CI doctor: enumerate every tool version pin and PASS/FAIL each one.
#
# Mirrors the pinned tool set in ops/ci/lib.sh + rust-toolchain.toml so
# the operator knows up-front when their local environment will drift
# from CI. Audit reference: HLT-038 ci.local-parity.lib-missing.
#
# Usage:
#   bash scripts/ci-doctor.sh
#
# Exits non-zero if any required tool is missing OR a pinned version
# does not match the local install.

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# shellcheck source=ops/ci/lib.sh
. "$ROOT/ops/ci/lib.sh"

# Track overall status without aborting on the first failure so the
# operator gets a complete report in one run.
overall=0

emit() {
    # emit <status> <tool> <detail>
    printf '%-4s %-14s %s\n' "$1" "$2" "$3"
}

fail() {
    overall=1
    emit FAIL "$1" "$2"
}

pass() {
    emit PASS "$1" "$2"
}

# ---- rust toolchain (read pin from rust-toolchain.toml) --------------------
check_rust() {
    local expected actual
    expected="$(grep -E '^[[:space:]]*channel' "$ROOT/rust-toolchain.toml" \
        | head -n1 \
        | sed -E 's/.*"([^"]+)".*/\1/')"
    if ! command -v rustc >/dev/null 2>&1; then
        fail rustc "missing (expected ${expected})"
        return
    fi
    actual="$(rustc --version 2>/dev/null | awk '{print $2}')"
    if [ "${actual}" = "${expected}" ]; then
        pass rustc "${actual} (matches rust-toolchain.toml)"
    else
        fail rustc "expected=${expected} actual=${actual}"
    fi
}

# ---- cargo-deny pinned to CI_CARGO_DENY_VERSION ----------------------------
check_cargo_deny() {
    local expected="${CI_CARGO_DENY_VERSION}" actual
    if ! command -v cargo-deny >/dev/null 2>&1; then
        fail cargo-deny "missing (expected ${expected}; install: cargo install cargo-deny --locked --version ${expected})"
        return
    fi
    actual="$(cargo-deny --version 2>/dev/null | awk '{print $2}')"
    if [ "${actual}" = "${expected}" ]; then
        pass cargo-deny "${actual}"
    else
        fail cargo-deny "expected=${expected} actual=${actual}"
    fi
}

# ---- gitleaks pinned to CI_GITLEAKS_VERSION --------------------------------
check_gitleaks() {
    local expected="${CI_GITLEAKS_VERSION}" actual
    if ! command -v gitleaks >/dev/null 2>&1; then
        fail gitleaks "missing (expected ${expected})"
        return
    fi
    # gitleaks version prints lines like:
    #   8.21.2
    # or:
    #   v8.21.2
    actual="$(gitleaks version 2>/dev/null | head -n1 | sed -E 's/^v//')"
    if [ "${actual}" = "${expected}" ]; then
        pass gitleaks "${actual}"
    else
        fail gitleaks "expected=${expected} actual=${actual}"
    fi
}

# ---- presence-only checks --------------------------------------------------
check_presence() {
    local tool="$1"
    if command -v "$tool" >/dev/null 2>&1; then
        pass "$tool" "$(command -v "$tool")"
    else
        fail "$tool" "missing (required for ci-local parity)"
    fi
}

# ---- python3 >= 3.10 -------------------------------------------------------
check_python() {
    if ! command -v python3 >/dev/null 2>&1; then
        fail python3 "missing (expected >=3.10)"
        return
    fi
    local actual major minor
    actual="$(python3 -c 'import sys; print("{}.{}.{}".format(*sys.version_info[:3]))' 2>/dev/null)"
    major="$(printf '%s' "$actual" | cut -d. -f1)"
    minor="$(printf '%s' "$actual" | cut -d. -f2)"
    if [ "${major:-0}" -gt 3 ] || { [ "${major:-0}" -eq 3 ] && [ "${minor:-0}" -ge 10 ]; }; then
        pass python3 "${actual} (>=3.10)"
    else
        fail python3 "expected>=3.10 actual=${actual}"
    fi
}

main() {
    printf 'ci-doctor: pinned-tool report (ops/ci/lib.sh + rust-toolchain.toml)\n'
    printf '%-4s %-14s %s\n' STAT TOOL DETAIL
    printf '%s\n' '----------------------------------------------------------'
    check_rust
    check_cargo_deny
    check_gitleaks
    check_presence jq
    check_presence curl
    check_presence just
    check_presence rtk
    check_python
    printf '%s\n' '----------------------------------------------------------'
    if [ "${overall}" -eq 0 ]; then
        printf 'ci-doctor: all required tools present and pinned\n'
    else
        printf 'ci-doctor: one or more tools missing or version-mismatched\n' >&2
    fi
    return "${overall}"
}

main "$@"
