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
readonly CI_REDLINEDB_RELEASE_TAG="${CI_REDLINEDB_RELEASE_TAG:-v1.0.1}"
readonly CI_REDLINEDB_RELEASE_ARTIFACT="${CI_REDLINEDB_RELEASE_ARTIFACT:-linux-x86_64}"
readonly CI_REDLINEDB_RELEASE_ASSET="${CI_REDLINEDB_RELEASE_ASSET:-redlinedb-${CI_REDLINEDB_RELEASE_TAG}-${CI_REDLINEDB_RELEASE_ARTIFACT}.tar.gz}"
readonly CI_REDLINEDB_RELEASE_BASE_URL="${CI_REDLINEDB_RELEASE_BASE_URL:-https://github.com/neverhuman/RedlineDB/releases/download/${CI_REDLINEDB_RELEASE_TAG}}"
readonly CI_REDLINEDB_RELEASE_URL="${CI_REDLINEDB_RELEASE_URL:-${CI_REDLINEDB_RELEASE_BASE_URL}/${CI_REDLINEDB_RELEASE_ASSET}}"
readonly CI_REDLINEDB_RELEASE_SHA256_URL="${CI_REDLINEDB_RELEASE_SHA256_URL:-${CI_REDLINEDB_RELEASE_URL}.sha256}"
readonly CI_JANKURAI_VERSION="${CI_JANKURAI_VERSION:-1.5.1}"
readonly CI_JANKURAI_GIT="${CI_JANKURAI_GIT:-https://github.com/neverhuman/jankurai.git}"
readonly CI_JANKURAI_TAG="${CI_JANKURAI_TAG:-v${CI_JANKURAI_VERSION}}"
readonly CI_JANKURAI_REV="${CI_JANKURAI_REV:-6f1aa45fca09ebb523f79b38ad465da28a86dfb1}"
readonly CI_JANKURAI_RELEASE_BASE_URL="${CI_JANKURAI_RELEASE_BASE_URL:-https://github.com/neverhuman/jankurai/releases/download/v${CI_JANKURAI_VERSION}}"
readonly CI_JANKURAI_LINUX_ASSET="${CI_JANKURAI_LINUX_ASSET:-jankurai-${CI_JANKURAI_VERSION}-x86_64-unknown-linux-gnu.tar.gz}"
readonly CI_JANKURAI_ASSET_URL="${CI_JANKURAI_ASSET_URL:-${CI_JANKURAI_RELEASE_BASE_URL}/${CI_JANKURAI_LINUX_ASSET}}"
readonly CI_JANKURAI_SHA256_URL="${CI_JANKURAI_SHA256_URL:-${CI_JANKURAI_ASSET_URL}.sha256}"
readonly CI_JANKURAI_SOURCE_ARCHIVE_URL="${CI_JANKURAI_SOURCE_ARCHIVE_URL:-https://github.com/neverhuman/jankurai/archive/refs/tags/v${CI_JANKURAI_VERSION}.tar.gz}"
readonly CI_JANKURAI_COMPILED_REPO_ROOT="${CI_JANKURAI_COMPILED_REPO_ROOT:-/home/runner/work/jankurai/jankurai}"
readonly CI_JANKURAI_LOCAL_RUNTIME_REPO_ROOT="${CI_JANKURAI_LOCAL_RUNTIME_REPO_ROOT:-/tmp/jankurai-v1.5.1-runtime-root00}"

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

# Install the pinned RedlineDB release package and print the CLI path on stdout.
# Status lines go to stderr so callers can safely capture the returned path.
ci_install_redlinedb_release() {
    local install_root="${CI_REDLINEDB_RELEASE_INSTALL_ROOT:-$PWD/target/ci/redlinedb-release/${CI_REDLINEDB_RELEASE_TAG}-${CI_REDLINEDB_RELEASE_ARTIFACT}}"
    local tmp_dir
    tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/redlinedb-release.XXXXXX")"

    curl -fsSL -o "$tmp_dir/$CI_REDLINEDB_RELEASE_ASSET" "$CI_REDLINEDB_RELEASE_URL"
    curl -fsSL -o "$tmp_dir/$CI_REDLINEDB_RELEASE_ASSET.sha256" "$CI_REDLINEDB_RELEASE_SHA256_URL"
    (
        cd "$tmp_dir"
        sha256sum -c "$CI_REDLINEDB_RELEASE_ASSET.sha256" >&2
    )

    tar -xzf "$tmp_dir/$CI_REDLINEDB_RELEASE_ASSET" -C "$tmp_dir"

    local package_dir
    package_dir="$tmp_dir/${CI_REDLINEDB_RELEASE_ASSET%.tar.gz}"
    if [ ! -x "$package_dir/bin/redlinedb" ]; then
        printf 'RedlineDB release asset missing executable: %s\n' \
            "$package_dir/bin/redlinedb" >&2
        return 1
    fi

    rm -rf "$install_root"
    mkdir -p "$install_root"
    cp -R "$package_dir/." "$install_root/"

    local version_output
    version_output="$("$install_root/bin/redlinedb" --version)"
    printf 'RedlineDB release asset verified: %s\n' "$CI_REDLINEDB_RELEASE_URL" >&2
    printf 'RedlineDB installed: %s (%s)\n' "$install_root/bin/redlinedb" "$version_output" >&2
    rm -rf "$tmp_dir"
    printf '%s\n' "$install_root/bin/redlinedb"
}

ci_verify_redlinedb_release_smoke() {
    local redlinedb_bin
    local output
    redlinedb_bin="$(ci_install_redlinedb_release)"
    output="$(printf 'SELECT 1;\n' | "$redlinedb_bin" -batch -bail -list -separator '|' :memory:)"
    if [ "$output" != "1" ]; then
        printf 'RedlineDB release smoke failed: expected `1`, got `%s`\n' "$output" >&2
        return 1
    fi
    printf 'RedlineDB release smoke passed: %s\n' "$redlinedb_bin" >&2
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

ci_install_jankurai_runtime_schemas() {
    local tmp_dir="$1"
    local binary_path="$2"
    local schema_root="$CI_JANKURAI_COMPILED_REPO_ROOT"
    local schema_dir
    local source_archive="$tmp_dir/jankurai-source-${CI_JANKURAI_VERSION}.tar.gz"
    local source_dir="$tmp_dir/jankurai-source"
    local source_root

    curl -fsSL -o "$source_archive" "$CI_JANKURAI_SOURCE_ARCHIVE_URL"
    mkdir -p "$source_dir"
    tar -xzf "$source_archive" -C "$source_dir"
    source_root="$(find "$source_dir" -mindepth 1 -maxdepth 1 -type d | head -n 1)"
    if [ -z "$source_root" ] || [ ! -d "$source_root/schemas" ]; then
        printf 'jankurai source archive missing schemas directory: %s\n' \
            "$CI_JANKURAI_SOURCE_ARCHIVE_URL" >&2
        return 1
    fi

    schema_dir="${schema_root}/schemas"
    if ! mkdir -p "${schema_root}/crates/jankurai" "$schema_dir" 2>/dev/null; then
        if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
            printf 'failed to create jankurai runtime schema path: %s\n' "$schema_dir" >&2
            return 1
        fi

        local compiled_root_len="${#CI_JANKURAI_COMPILED_REPO_ROOT}"
        local runtime_root_len="${#CI_JANKURAI_LOCAL_RUNTIME_REPO_ROOT}"
        local compiled_root_matches
        local runtime_root_matches
        if [ "$compiled_root_len" -ne "$runtime_root_len" ]; then
            printf 'jankurai local runtime root must match compiled root length: %s != %s\n' \
                "$runtime_root_len" "$compiled_root_len" >&2
            return 1
        fi
        compiled_root_matches="$(
            LC_ALL=C grep -a -o -F "$CI_JANKURAI_COMPILED_REPO_ROOT" "$binary_path" \
                | wc -l \
                | tr -d ' '
        )"
        if [ "${compiled_root_matches:-0}" -eq 0 ]; then
            printf 'jankurai binary missing compiled schema root: %s\n' \
                "$CI_JANKURAI_COMPILED_REPO_ROOT" >&2
            return 1
        fi
        OLD_JANKURAI_ROOT="$CI_JANKURAI_COMPILED_REPO_ROOT" \
            NEW_JANKURAI_ROOT="$CI_JANKURAI_LOCAL_RUNTIME_REPO_ROOT" \
            perl -0pi -e 's/\Q$ENV{OLD_JANKURAI_ROOT}\E/$ENV{NEW_JANKURAI_ROOT}/g' \
            "$binary_path"
        runtime_root_matches="$(
            LC_ALL=C grep -a -o -F "$CI_JANKURAI_LOCAL_RUNTIME_REPO_ROOT" "$binary_path" \
                | wc -l \
                | tr -d ' '
        )"
        if [ "$runtime_root_matches" -ne "$compiled_root_matches" ]; then
            printf 'jankurai local schema root relocation mismatch: %s -> %s\n' \
                "$compiled_root_matches" "$runtime_root_matches" >&2
            return 1
        fi

        schema_root="$CI_JANKURAI_LOCAL_RUNTIME_REPO_ROOT"
        schema_dir="${schema_root}/schemas"
        mkdir -p "${schema_root}/crates/jankurai" "$schema_dir"
        printf 'jankurai local schema root relocated: %s -> %s (matches=%s)\n' \
            "$CI_JANKURAI_COMPILED_REPO_ROOT" "$CI_JANKURAI_LOCAL_RUNTIME_REPO_ROOT" \
            "$runtime_root_matches"
    fi

    find "$schema_dir" -type f -name '*.schema.json' -delete
    install -m 0644 "$source_root"/schemas/*.schema.json "$schema_dir/"
    if [ ! -s "$schema_dir/proofbind-witness.schema.json" ]; then
        printf 'jankurai runtime schemas missing proofbind witness schema: %s\n' \
            "$schema_dir/proofbind-witness.schema.json" >&2
        return 1
    fi

    printf 'jankurai runtime schemas installed: %s\n' "$schema_dir"
    printf 'jankurai runtime schemas source: %s\n' "$CI_JANKURAI_SOURCE_ARCHIVE_URL"
}

# Install the pinned upstream jankurai release binary. The tag provenance
# check stays in place, but CI/local gates consume the reviewed release asset
# instead of rebuilding jankurai from source.
ci_install_jankurai() {
    ci_verify_jankurai_source

    local install_dir
    install_dir="${CARGO_HOME:-$HOME/.cargo}/bin"
    mkdir -p "$install_dir"
    export PATH="$install_dir:$PATH"

    local tmp_dir
    tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/jankurai-release.XXXXXX")"

    curl -fsSL -o "$tmp_dir/$CI_JANKURAI_LINUX_ASSET" "$CI_JANKURAI_ASSET_URL"
    curl -fsSL -o "$tmp_dir/$CI_JANKURAI_LINUX_ASSET.sha256" "$CI_JANKURAI_SHA256_URL"
    (
        cd "$tmp_dir"
        sha256sum -c "$CI_JANKURAI_LINUX_ASSET.sha256"
    )

    tar -xzf "$tmp_dir/$CI_JANKURAI_LINUX_ASSET" -C "$tmp_dir"

    local extracted_binary
    extracted_binary="$tmp_dir/${CI_JANKURAI_LINUX_ASSET%.tar.gz}/jankurai"
    if [ ! -x "$extracted_binary" ]; then
        printf 'jankurai release asset missing executable: %s\n' "$extracted_binary" >&2
        return 1
    fi

    ci_install_jankurai_runtime_schemas "$tmp_dir" "$extracted_binary"
    install -m 0755 "$extracted_binary" "$install_dir/jankurai"
    hash -r 2>/dev/null || true

    local version_output
    version_output="$(jankurai --version)"
    case "$version_output" in
        "jankurai ${CI_JANKURAI_VERSION}"*) ;;
        *)
            printf 'installed jankurai version mismatch: got %s, expected %s\n' \
                "$version_output" "$CI_JANKURAI_VERSION" >&2
            return 1
            ;;
    esac
    printf 'jankurai release asset verified: %s\n' "$CI_JANKURAI_ASSET_URL"
    printf 'jankurai installed: %s (%s)\n' "$(command -v jankurai)" "$version_output"
    rm -rf "$tmp_dir"
}

ci_install_jankurai_logged() {
    local log_path="$1"
    mkdir -p "$(dirname "$log_path")"

    if ! ci_install_jankurai >"$log_path" 2>&1; then
        cat "$log_path"
        return 1
    fi

    cat "$log_path"
}
