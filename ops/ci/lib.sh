#!/usr/bin/env bash
# Shared CI helper library.
#
# Sourced by every CI lane (both GitHub Actions workflows and the local
# `just` lanes) to share artifact-assertion helpers and tool version pins
# so the same gates run locally and in CI. Audit reference:
# HLT-038 ci.local-parity.lib-missing.
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
readonly CI_JANKURAI_GIT="${CI_JANKURAI_GIT:-https://github.com/jepsontaylor/jankurai}"

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

# ---- Tool-pin parity helpers ------------------------------------------------
# Lets local `just` recipes confirm they are about to run the same tool
# version CI runs. If the local install drifts, the helper prints the
# expected pin and the command to install it, then exits non-zero.

ci_require_pin() {
    local tool="$1" expected="$2" actual="$3" install_cmd="$4"
    if [ "$actual" != "$expected" ]; then
        printf 'tool pin drift: %s expected=%s actual=%s\n' "$tool" "$expected" "$actual" >&2
        printf 'install via: %s\n' "$install_cmd" >&2
        return 1
    fi
}
