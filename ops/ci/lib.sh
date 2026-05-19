#!/usr/bin/env bash
# Shared CI helper library.
#
# Sourced by every CI lane (both GitHub Actions workflows and the local
# `just` lanes) to share artifact-assertion helpers and tool version pins
# so the same gates run locally and in CI. Audit reference:
# HLT-042 ci-local-parity (lib-missing), HLT-034 ci-bad-behavior.
#
# Usage:
#   . ops/ci/lib.sh
#
# This file is intentionally pure bash with no GitHub-Actions-specific
# behaviour; it must run unchanged inside `just` recipes and `cargo`
# wrappers.

set -euo pipefail

# ---- Pinned tool versions ---------------------------------------------------
# Bump in lockstep with the matching `.github/workflows/*.yml` pin so the
# local proof lane and the CI proof lane agree on the artifact.

readonly CI_RUST_TOOLCHAIN="${CI_RUST_TOOLCHAIN:-1.95.0}"
readonly CI_CARGO_DENY_VERSION="${CI_CARGO_DENY_VERSION:-0.18.0}"
readonly CI_GITLEAKS_VERSION="${CI_GITLEAKS_VERSION:-8.21.2}"
readonly CI_JANKURAI_GIT="${CI_JANKURAI_GIT:-https://github.com/neverhuman/jankurai.git}"
readonly CI_JANKURAI_TAG="${CI_JANKURAI_TAG:-v1.5.1}"
readonly CI_JANKURAI_REV="${CI_JANKURAI_REV:-6f1aa45fca09ebb523f79b38ad465da28a86dfb1}"

# ---- Artifact assertions ----------------------------------------------------
# Every CI lane that produces an evidence artifact should call
# `ci_assert_artifact <path>` immediately after producing it. Fails fast
# with a clear error if the file is missing or zero-byte, so a silent
# upstream failure surfaces as a CI failure rather than an empty upload.

ci_assert_artifact() {
    local path="$1"
    if [ ! -s "$path" ]; then
        printf '::error file=%s::missing or empty CI evidence artifact\n' "$path" >&2
        return 1
    fi
}

# Walks every path passed in and asserts each one. Use in upload-artifact
# pre-flight steps to fail loudly when an upstream job dropped a file.

ci_assert_artifacts() {
    local path
    for path in "$@"; do
        ci_assert_artifact "$path"
    done
}

# ---- Soft-gate runner -------------------------------------------------------
# Run a command but never propagate its non-zero exit; instead log a
# machine-grep-able marker line. This is the mechanism that replaces
# `continue-on-error: true` in the workflow YAML: the soft-gate semantics
# live here, the workflow YAML is hard-gated end-to-end.
#
# Every call MUST cite an entry in .jankurai/ci-soft-gate-ledger.toml so the
# soft gate is auditable. The ledger entry name is passed as the first
# argument and stamped into the log marker.
#
# Usage:
#   ci_soft_gate "ledger-entry-name" "/path/to/log" -- cmd arg arg ...
ci_soft_gate() {
    local entry="$1"
    local log_path="$2"
    shift 2
    if [ "${1:-}" = "--" ]; then
        shift
    fi
    mkdir -p "$(dirname "$log_path")"
    local rc=0
    "$@" >>"$log_path" 2>&1 || rc=$?
    if [ "$rc" -eq 0 ]; then
        printf 'soft-gate=%s status=passed exit=0\n' "$entry" | tee -a "$log_path"
    else
        printf 'soft-gate=%s status=soft-failed exit=%d ledger=.jankurai/ci-soft-gate-ledger.toml\n' \
            "$entry" "$rc" | tee -a "$log_path" >&2
    fi
    # Always return 0: soft-gate semantics. The ledger row + log marker
    # are the auditable evidence that this failure was non-blocking by
    # design.
    return 0
}

# Verify the pinned upstream tag resolves to the exact commit we expect
# before any install path uses it.
ci_verify_jankurai_source() {
    local resolved_rev
    resolved_rev="$(
        git ls-remote "${CI_JANKURAI_GIT}" "refs/tags/${CI_JANKURAI_TAG}^{}" \
            | awk 'NR == 1 { print $1 }' || true
    )"

    if [ -z "$resolved_rev" ]; then
        printf 'expected jankurai tag %s to resolve at %s\n' \
            "$CI_JANKURAI_TAG" "$CI_JANKURAI_GIT" >&2
        return 1
    fi

    if [ "$resolved_rev" != "$CI_JANKURAI_REV" ]; then
        printf 'jankurai tag %s resolved to %s, expected %s\n' \
            "$CI_JANKURAI_TAG" "$resolved_rev" "$CI_JANKURAI_REV" >&2
        return 1
    fi
}

# Install the pinned upstream jankurai release directly from the git tag.
ci_install_jankurai() {
    ci_verify_jankurai_source
    cargo install --git "${CI_JANKURAI_GIT}" --tag "${CI_JANKURAI_TAG}" --locked jankurai
}
