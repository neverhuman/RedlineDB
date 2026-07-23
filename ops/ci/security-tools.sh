#!/usr/bin/env bash

readonly REDLINE_CARGO_AUDIT_VERSION="cargo-audit 0.22.1"
readonly REDLINE_CARGO_AUDIT_SHA256="1a17ff4c0449d1924aacda8dd20c06dccc3cceeed4dd17a71523f672bf97b70b"
readonly REDLINE_CARGO_DENY_VERSION="cargo-deny 0.19.8"
readonly REDLINE_CARGO_DENY_SHA256="ef27c757f50d77c5c2d9114fbc6ad45d2b8903506cead473a70b8ee659ea7a18"
readonly REDLINE_GITLEAKS_VERSION="gitleaks version 8.21.2"
readonly REDLINE_GITLEAKS_SHA256="50b742abd7daad8bbddb6301f3017efb680632d9a5b3b4d8f137b3aac250e359"
readonly REDLINE_SYFT_VERSION="syft 1.40.0"
readonly REDLINE_SYFT_SHA256="eb9714fb8e4b8f2a647e7bb312f1e0b9f83a7aa30418658bf46583cfa83d27d2"
readonly REDLINE_ACTIONLINT_VERSION="1.7.8"
readonly REDLINE_ACTIONLINT_SHA256="9ab20f97947e525d92175a7029eba4fe62749b49e556a735862dac037dd6f8dd"

redline_validate_security_tool() {
    local name="${1:?tool name is required}"
    local path="${2:?tool path is required}"
    local expected_version="${3:?tool version is required}"
    local expected_sha256="${4:?tool digest is required}"
    local actual_version actual_sha256 mode_owner
    [[ "$path" == /* && -f "$path" && ! -L "$path" && -x "$path" \
        && "$(realpath -e -- "$path")" == "$path" \
        && "$(stat -c '%h' -- "$path")" == 1 ]] || {
        printf 'security tool is not a physical single-link executable: %s=%s\n' \
            "$name" "$path" >&2
        return 1
    }
    actual_sha256="$(sha256sum "$path" | awk '{print $1}')"
    [[ "$actual_sha256" == "$expected_sha256" ]] || {
        printf 'security tool digest mismatch: %s expected=%s got=%s\n' \
            "$name" "$expected_sha256" "$actual_sha256" >&2
        return 1
    }
    actual_version="$("$path" --version 2>/dev/null | head -n 1)" || return 1
    [[ "$actual_version" == "$expected_version" \
        || "$actual_version" == "$expected_version"* ]] || {
        printf 'security tool version mismatch: %s expected=%s got=%s\n' \
            "$name" "$expected_version" "$actual_version" >&2
        return 1
    }
    if [[ "${JAIN_RELEASE_CI:-0}" == 1 ]]; then
        mode_owner="$(stat -c '%a:%u:%g' -- "$path")"
        [[ "$mode_owner" == 555:0:0 ]] || {
            printf 'release security tool custody mismatch: %s mode:uid:gid=%s\n' \
                "$name" "$mode_owner" >&2
            return 1
        }
    fi
}

redline_resolve_security_tools() {
    if [[ "${JAIN_RELEASE_CI:-0}" == 1 ]]; then
        REDLINE_CARGO_AUDIT_BIN="${JAIN_REAL_CARGO_AUDIT:-}"
        REDLINE_CARGO_DENY_BIN="${JAIN_REAL_CARGO_DENY:-}"
        REDLINE_GITLEAKS_BIN="${JAIN_REAL_GITLEAKS:-$(type -P gitleaks 2>/dev/null || true)}"
        REDLINE_SYFT_BIN="${JAIN_REAL_SYFT:-$(type -P syft 2>/dev/null || true)}"
        REDLINE_ACTIONLINT_BIN="${JAIN_REAL_ACTIONLINT:-$(type -P actionlint 2>/dev/null || true)}"
    else
        REDLINE_CARGO_AUDIT_BIN="${JAIN_REAL_CARGO_AUDIT:-${HOME:?HOME is required}/.cargo/bin/cargo-audit}"
        REDLINE_CARGO_DENY_BIN="${JAIN_REAL_CARGO_DENY:-$HOME/.cargo/bin/cargo-deny}"
        REDLINE_GITLEAKS_BIN="${JAIN_REAL_GITLEAKS:-$HOME/.cargo/bin/gitleaks}"
        REDLINE_SYFT_BIN="${JAIN_REAL_SYFT:-$HOME/.local/bin/syft}"
        REDLINE_ACTIONLINT_BIN="${JAIN_REAL_ACTIONLINT:-$HOME/.local/bin/actionlint}"
    fi
    redline_validate_security_tool cargo-audit "$REDLINE_CARGO_AUDIT_BIN" \
        "$REDLINE_CARGO_AUDIT_VERSION" "$REDLINE_CARGO_AUDIT_SHA256"
    redline_validate_security_tool cargo-deny "$REDLINE_CARGO_DENY_BIN" \
        "$REDLINE_CARGO_DENY_VERSION" "$REDLINE_CARGO_DENY_SHA256"
    redline_validate_security_tool gitleaks "$REDLINE_GITLEAKS_BIN" \
        "$REDLINE_GITLEAKS_VERSION" "$REDLINE_GITLEAKS_SHA256"
    redline_validate_security_tool syft "$REDLINE_SYFT_BIN" \
        "$REDLINE_SYFT_VERSION" "$REDLINE_SYFT_SHA256"
    redline_validate_security_tool actionlint "$REDLINE_ACTIONLINT_BIN" \
        "$REDLINE_ACTIONLINT_VERSION" "$REDLINE_ACTIONLINT_SHA256"
    export REDLINE_CARGO_AUDIT_BIN REDLINE_CARGO_DENY_BIN REDLINE_GITLEAKS_BIN
    export REDLINE_SYFT_BIN REDLINE_ACTIONLINT_BIN
}
