#!/usr/bin/env bash
# CI doctor: enumerate every tool version pin and PASS/FAIL each one.
#
# Mirrors the pinned tool set in ops/ci/lib.sh + rust-toolchain.toml so
# the operator knows up-front when their local environment will drift
# from CI. Audit reference: HLT-042 ci-local-parity.doctor-missing.
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

overall=0

emit() {
    printf '%-4s %-14s %s\n' "$1" "$2" "$3"
}

fail() {
    overall=1
    emit FAIL "$1" "$2"
}

pass() {
    emit PASS "$1" "$2"
}

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

check_nextest() {
    local expected="0.9.133" actual
    if ! cargo nextest --version >/dev/null 2>&1; then
        fail cargo-nextest "missing (expected ${expected}; install: curl -LsSf https://get.nexte.st/${expected}/mac | tar zxf - -C \${CARGO_HOME:-~/.cargo}/bin)"
        return
    fi
    actual="$(cargo nextest --version 2>/dev/null | head -n1 | awk '{print $2}')"
    if [ "${actual}" = "${expected}" ]; then
        pass cargo-nextest "${actual}"
    else
        fail cargo-nextest "expected=${expected} actual=${actual} (run: cargo install cargo-nextest --locked)"
    fi
}

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

check_gitleaks() {
    local expected="${CI_GITLEAKS_VERSION}" actual
    if ! command -v gitleaks >/dev/null 2>&1; then
        fail gitleaks "missing (expected ${expected})"
        return
    fi
    actual="$(gitleaks version 2>/dev/null | head -n1 | sed -E 's/^v//')"
    if [ "${actual}" = "${expected}" ]; then
        pass gitleaks "${actual}"
    else
        fail gitleaks "expected=${expected} actual=${actual}"
    fi
}

check_presence() {
    local tool="$1"
    if command -v "$tool" >/dev/null 2>&1; then
        pass "$tool" "$(command -v "$tool")"
    else
        fail "$tool" "missing (required for ci-local parity)"
    fi
}

check_mold() {
    # mold is required on Linux (see .cargo/config.toml [target.x86_64-unknown-linux-gnu])
    case "$(uname -s)" in
      Linux)
        if command -v mold >/dev/null 2>&1; then
            pass mold "$(mold --version 2>/dev/null | head -n1)"
        else
            fail mold "missing on Linux (install: sudo apt-get install mold)"
        fi
        ;;
      *)
        pass mold "n/a (not required on $(uname -s))"
        ;;
    esac
}

check_required_dispatch() {
    local required_block expected
    required_block="$(sed -n '/^[[:space:]]*required)/,/^[[:space:]]*;;/p' "$ROOT/scripts/ci-local.sh")"
    expected='bash "$ROOT/ops/ci/pr-ci.sh"'
    if printf '%s\n' "$required_block" | grep -Fq "$expected"; then
        pass ci-required "scripts/ci-local.sh -> ops/ci/pr-ci.sh"
    else
        fail ci-required "required must dispatch directly to ops/ci/pr-ci.sh"
    fi
}

main() {
    printf 'ci-doctor: pinned-tool report (ops/ci/lib.sh + rust-toolchain.toml)\n'
    printf '%-4s %-14s %s\n' STAT TOOL DETAIL
    printf '%s\n' '----------------------------------------------------------'
    check_rust
    check_nextest
    check_cargo_deny
    check_gitleaks
    check_mold
    check_presence jq
    check_presence curl
    check_presence just
    check_presence rtk
    check_required_dispatch
    printf '%s\n' '----------------------------------------------------------'
    if [ "${overall}" -eq 0 ]; then
        printf 'ci-doctor: all required tools present and pinned\n'
    else
        printf 'ci-doctor: one or more tools missing or version-mismatched\n' >&2
    fi
    return "${overall}"
}

main "$@"
