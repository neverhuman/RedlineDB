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
readonly CI_CARGO_DENY_VERSION="${CI_CARGO_DENY_VERSION:-0.19.8}"
readonly CI_GITLEAKS_VERSION="${CI_GITLEAKS_VERSION:-8.21.2}"
readonly CI_REDLINEDB_RELEASE_TAG="${CI_REDLINEDB_RELEASE_TAG:-v4.2.0}"
readonly CI_REDLINEDB_RELEASE_ARTIFACT="${CI_REDLINEDB_RELEASE_ARTIFACT:-linux-x86_64}"
readonly CI_REDLINEDB_RELEASE_ASSET="${CI_REDLINEDB_RELEASE_ASSET:-redlinedb-${CI_REDLINEDB_RELEASE_TAG}-${CI_REDLINEDB_RELEASE_ARTIFACT}.tar.gz}"
readonly CI_REDLINEDB_RELEASE_URL="${CI_REDLINEDB_RELEASE_URL:-}"
readonly CI_REDLINEDB_RELEASE_SHA256_URL="${CI_REDLINEDB_RELEASE_SHA256_URL:-${CI_REDLINEDB_RELEASE_URL}.sha256}"
CI_REDLINE_TESTING_VERSION="${CI_REDLINE_TESTING_VERSION:-latest}"
CI_REDLINE_TESTING_EXPECTED_TARBALL_SHA256="${CI_REDLINE_TESTING_EXPECTED_TARBALL_SHA256:-}"
CI_REDLINE_TESTING_EXPECTED_BINARY_SHA256="${CI_REDLINE_TESTING_EXPECTED_BINARY_SHA256:-}"
readonly CI_REDLINE_TESTING_ATTESTATION_REPO="${CI_REDLINE_TESTING_ATTESTATION_REPO:-neverhuman/redline-testing}"

# BEGIN GENERATED JANKURAI PIN — DO NOT EDIT
export JERYU_JANKURAI_SOURCE_REPO="http://127.0.0.1:8787/git/jeryu/jankurai.git"
export JERYU_JANKURAI_VERSION="jankurai 1.6.11"
export JERYU_JANKURAI_SHA256="96d99e6e7d8dc9cf23df1081edd1f975231456592f81d9405385219a2c7298aa"
export JERYU_JANKURAI_SOURCE_REV="4dfbdfa3585f1928d5f996d7b5e14608dff14a03"
export JERYU_JANKURAI_SOURCE_TAG="v1.6.11-deadlang-precision-split.2"
export JERYU_JANKURAI_SOURCE_TREE="7e5d501aa6f0ee6ced9a48c6288a9943d0b9573c"
export JERYU_JANKURAI_SOURCE_ARCHIVE_SHA256="1aa3d178dec0fbb8d0657dd465ea6fda830ffc4ec1f65560b7b7d1682fd87e69"
export JERYU_JANKURAI_CARGO_LOCK_SHA256="b9acb981c326226a687d0b6703e4f7ee303148e9e1a6dda1aa03d77988820f6a"
export JERYU_JANKURAI_RUST_TOOLCHAIN="1.95.0"
export JERYU_JANKURAI_RUSTC_VERSION="rustc 1.95.0 (59807616e 2026-04-14)"
export JERYU_JANKURAI_CARGO_VERSION="cargo 1.95.0 (f2d3ce0bd 2026-03-21)"
export JERYU_JANKURAI_TARGET_TRIPLE="x86_64-unknown-linux-gnu"
export JERYU_JANKURAI_BUILD_MODE="cargo-install-locked-offline-path-v1"
# END GENERATED JANKURAI PIN

# Consumer authority for the generated block above: protected Jeryu Tool main
# at the immutable split.3 release point. Installation receipts may predate
# this commit only when they bind the byte-identical manifest digest.
readonly JERYU_TOOL_AUTHORITY_REPO="http://127.0.0.1:8787/git/jeryu/jeryu-tool.git"
readonly JERYU_TOOL_AUTHORITY_COMMIT="479489f56f42045a71bf0651c3793d82f8689630"
readonly JERYU_TOOL_AUTHORITY_TREE="2d97c2fe1d146770b97e2905048ac5d4f537dc4a"
readonly JERYU_TOOL_AUTHORITY_TAG="jeryu-tool-v5.1.0-split.3"
readonly JERYU_TOOL_MANIFEST_SHA256="7aaac7f1b8c1543eba5215ec7cd2bf35e0c2411339ed9af1d1a6ace68d2807d8"

require_jankurai() {
    local mode=receipt-bound
    local expected_broker="/opt/jain-ci/authority/release-bin/jankurai"
    local expected_governed="/home/ubuntu/.jeryu/bin/jankurai"
    local bin bin_dir governed_root normalized resolved actual actual_sha receipt receipt_digest receipt_sha
    local expected_test=false expected_verification=release-authoritative
    local expected_governance=governed expected_protected=true
    local expected_protection=immutable-main-v1 found_receipt=0
    local -a receipt_candidates=()
    if [[ "${JAIN_RELEASE_CI:-0}" == "1" ]]; then
        mode=release-broker
        resolved="$(type -P jankurai 2>/dev/null || true)"
        if [[ "$resolved" != "$expected_broker" ]]; then
            printf 'release broker Jankurai path mismatch: expected %s, resolved %s\n' \
                "$expected_broker" "${resolved:-missing}" >&2
            return 1
        fi
        bin="$resolved"
    else
        bin="${JERYU_GOVERNED_JANKURAI_BIN:-$expected_governed}"
    fi
    if [[ "$bin" != /* || ! -f "$bin" || -L "$bin" || ! -x "$bin" ]]; then
        printf 'governed jankurai must be an absolute executable regular file: %s\n' "$bin" >&2
        return 1
    fi
    normalized="$(realpath -m "$bin")"
    if [[ "$normalized" != "$bin" ]]; then
        printf 'governed jankurai path traverses a symlink: %s -> %s\n' \
            "$bin" "$normalized" >&2
        return 1
    fi
    if [[ "$mode" == release-broker \
        && "$(stat -c '%u:%g:%a:%h' -- "$bin" 2>/dev/null || true)" != "0:0:555:1" ]]; then
        printf 'release broker Jankurai custody mismatch: expected root:root, mode 0555, and one link at %s\n' \
            "$bin" >&2
        return 1
    fi
    if [[ "$mode" != release-broker ]]; then
        bin_dir="$(dirname "$bin")"
        export PATH="$bin_dir:$PATH"
        resolved="$(type -P jankurai 2>/dev/null || true)"
        if [[ "$resolved" != "$bin" ]]; then
            printf 'governed jankurai shadowed: expected %s, resolved %s\n' \
                "$bin" "${resolved:-missing}" >&2
            return 1
        fi
    fi
    actual="$("$bin" --version 2>/dev/null || true)"
    actual_sha="$(sha256sum "$bin" 2>/dev/null | awk '{print $1}')"
    if [[ "$actual" != "$JERYU_JANKURAI_VERSION" \
        || "$actual_sha" != "$JERYU_JANKURAI_SHA256" ]]; then
        printf 'governed jankurai identity mismatch at %s: version=%s sha256=%s\n' \
            "$bin" "${actual:-missing}" "${actual_sha:-missing}" >&2
        return 1
    fi
    export JERYU_GOVERNED_JANKURAI_BIN="$bin"
    if [[ "$mode" == release-broker ]]; then
        if [[ -n "${JERYU_JANKURAI_RECEIPT:-}" \
            || -n "${JERYU_JANKURAI_RECEIPT_SHA256:-}" \
            || "${JERYU_JANKURAI_ALLOW_TEST_RECEIPT:-0}" != "0" ]]; then
            printf 'release broker Jankurai rejects caller receipt authority\n' >&2
            return 1
        fi
        found_receipt=1
    elif [[ "${JERYU_JANKURAI_ALLOW_TEST_RECEIPT:-0}" == "1" ]]; then
        expected_test=true
        expected_verification=diagnostic-candidate
        expected_governance=diagnostic-candidate
        expected_protected=false
        expected_protection=not-applicable
    fi
    if [[ "$mode" == release-broker ]]; then
        receipt_candidates=()
    elif [[ -n "${JERYU_JANKURAI_RECEIPT:-}" ]]; then
        receipt_candidates=("$JERYU_JANKURAI_RECEIPT")
    elif [[ "$bin" == "$expected_governed" ]]; then
        governed_root="$(dirname "$(dirname "$expected_governed")")"
        receipt_candidates=("$governed_root"/receipts/jankurai/sha256/*.json)
    else
        printf 'non-governed jankurai requires an explicit installation receipt: %s\n' \
            "$bin" >&2
        return 1
    fi
    for receipt in "${receipt_candidates[@]}"; do
        [[ -f "$receipt" ]] || continue
        receipt_digest="$(basename "$receipt" .json)"
        [[ "$receipt_digest" =~ ^[0-9a-f]{64}$ ]] || continue
        receipt_sha="$(sha256sum "$receipt" | awk '{print $1}')"
        [[ "$receipt_sha" == "$receipt_digest" ]] || continue
        if jq -e \
            --arg remote "$JERYU_JANKURAI_SOURCE_REPO" \
            --arg commit "$JERYU_JANKURAI_SOURCE_REV" \
            --arg tag "$JERYU_JANKURAI_SOURCE_TAG" \
            --arg tree "$JERYU_JANKURAI_SOURCE_TREE" \
            --arg archive "$JERYU_JANKURAI_SOURCE_ARCHIVE_SHA256" \
            --arg lock "$JERYU_JANKURAI_CARGO_LOCK_SHA256" \
            --arg rustc "$JERYU_JANKURAI_RUSTC_VERSION" \
            --arg cargo "$JERYU_JANKURAI_CARGO_VERSION" \
            --arg triple "$JERYU_JANKURAI_TARGET_TRIPLE" \
            --arg mode "$JERYU_JANKURAI_BUILD_MODE" \
            --arg digest "$JERYU_JANKURAI_SHA256" \
            --arg version "$JERYU_JANKURAI_VERSION" \
            --arg path "$bin" \
            --arg manifest_repo "$JERYU_TOOL_AUTHORITY_REPO" \
            --arg manifest_sha "$JERYU_TOOL_MANIFEST_SHA256" \
            --arg verification "$expected_verification" \
            --arg governance "$expected_governance" \
            --arg protection "$expected_protection" \
            --argjson protected_main "$expected_protected" \
            --argjson test_mode "$expected_test" \
            '.schema == "jeryu.jankurai-installation/v1" and
             .source.remote == $remote and .source.commit == $commit and .source.tag == $tag and
             .source.tree == $tree and .source.archive_sha256 == $archive and
             .source.cargo_lock_sha256 == $lock and .source.verification == $verification and
             .build.rustc == $rustc and .build.cargo == $cargo and
             .build.target_triple == $triple and .build.mode == $mode and
             .build.cargo_net_offline == true and .build.dedicated_cargo_home == true and
             .build.git_global_config_disabled == true and .build.git_system_config_disabled == true and
             .build.git_http_follow_redirects == false and .build.git_terminal_prompt == false and
             .build.jankurai_update_check == false and
             .build.network_scope == "local-forge-source-plus-offline-cargo" and
             .build.no_proxy == "127.0.0.1,localhost,::1" and
             .governance.status == $governance and
             .governance.manifest_repo == $manifest_repo and
             (.governance.manifest_commit | test("^[0-9a-f]{40}$")) and
             (.governance.manifest_tree | test("^[0-9a-f]{40}$")) and
             .governance.manifest_sha256 == $manifest_sha and
             .governance.protected_main == $protected_main and
             .governance.protection_policy == $protection and
             .binary.sha256 == $digest and .binary.version_output == $version and
             .installation.path == $path and .installation.atomic == true and
             .test_mode == $test_mode and .conclusion == "success"' "$receipt" >/dev/null; then
            export JERYU_JANKURAI_RECEIPT="$receipt"
            export JERYU_JANKURAI_RECEIPT_SHA256="$receipt_digest"
            found_receipt=1
            break
        fi
    done
    if [[ "$mode" != release-broker && "$found_receipt" -ne 1 ]]; then
        printf 'governed jankurai receipt mismatch: binary=%s test_mode=%s\n' \
            "$bin" "$expected_test" >&2
        return 1
    fi
    export JANKURAI_NO_UPDATE_CHECK=1 GIT_TERMINAL_PROMPT=0
}

# Keep literal `jankurai` commands visible to the adoption auditor while every
# invocation revalidates the current protected Tool authority.
jankurai() {
    require_jankurai || return 1
    "$JERYU_GOVERNED_JANKURAI_BIN" "$@"
}

ci_redline_testing_version_from_tag() {
    local tag="${1:?release tag required}"
    case "$tag" in
        v*) printf '%s\n' "${tag#v}" ;;
        *) printf '%s\n' "$tag" ;;
    esac
}

ci_redline_testing_version_from_artifact() {
    local artifact="${1:?release artifact required}"
    local version
    version="$(printf '%s\n' "$artifact" | sed -n 's/^redline-testing-\(.*\)-linux-x86_64\.tar\.gz$/\1/p')"
    if [ -z "$version" ]; then
        return 1
    fi
    printf '%s\n' "$version"
}

ci_resolve_redline_testing_release() {
    local requested_version="${CI_REDLINE_TESTING_VERSION:-latest}"
    if [ -n "${CI_REDLINE_TESTING_REQUESTED_VERSION:-}" ]; then
        requested_version="$CI_REDLINE_TESTING_REQUESTED_VERSION"
    else
        CI_REDLINE_TESTING_REQUESTED_VERSION="$requested_version"
    fi

    if [ -n "${CI_REDLINE_TESTING_URL:-}" ]; then
        local override_artifact="${CI_REDLINE_TESTING_ARTIFACT:-${CI_REDLINE_TESTING_URL##*/}}"
        local override_version
        override_version="$(ci_redline_testing_version_from_artifact "$override_artifact" 2>/dev/null || true)"
        if [ -z "$override_version" ]; then
            override_version="${requested_version:-latest}"
            override_version="${override_version#v}"
        fi
        CI_REDLINE_TESTING_VERSION="$override_version"
        CI_REDLINE_TESTING_RELEASE_TAG="${CI_REDLINE_TESTING_RELEASE_TAG:-v$override_version}"
        CI_REDLINE_TESTING_ARTIFACT="$override_artifact"
        CI_REDLINE_TESTING_BASE_URL="${CI_REDLINE_TESTING_BASE_URL:-${CI_REDLINE_TESTING_URL%/$override_artifact}}"
        CI_REDLINE_TESTING_SHA256_URL="${CI_REDLINE_TESTING_SHA256_URL:-${CI_REDLINE_TESTING_URL}.sha256}"
        CI_REDLINE_TESTING_RELEASE_MANIFEST_URL="${CI_REDLINE_TESTING_RELEASE_MANIFEST_URL:-${CI_REDLINE_TESTING_BASE_URL}/release-manifest.json}"
        return 0
    fi

    local release_json
    local artifact_name
    local release_tag
    local release_version

    if [ "$requested_version" = "latest" ]; then
        while IFS= read -r release_json; do
            release_tag="$(jq -r '.tag_name // empty' <<<"$release_json")"
            [ -n "$release_tag" ] || continue
            artifact_name="$(
                jq -r '
                    .assets[]
                    | .name
                    | select(test("^redline-testing-[0-9A-Za-z.+-]+-linux-x86_64\\.tar\\.gz$"))
                ' <<<"$release_json" | head -n 1
            )"
            [ -n "$artifact_name" ] || continue
            release_version="$(ci_redline_testing_version_from_artifact "$artifact_name")" || continue
            if [ "$release_version" != "$(ci_redline_testing_version_from_tag "$release_tag")" ]; then
                continue
            fi
            if ! jq -e --arg artifact "$artifact_name" '
                .assets | any(.name == $artifact) and any(.name == ($artifact + ".sha256"))
            ' <<<"$release_json" >/dev/null; then
                continue
            fi
            break
        done < <(
            gh api "repos/${CI_REDLINE_TESTING_ATTESTATION_REPO}/releases?per_page=100" --paginate \
                | jq -s -c 'add | map(select((.draft | not) and (.prerelease | not))) | .[]'
        )
        if [ -z "${release_json:-}" ] || [ -z "${artifact_name:-}" ] || [ -z "${release_tag:-}" ]; then
            printf 'unable to resolve latest redline-testing release with required assets from %s\n' \
                "$CI_REDLINE_TESTING_ATTESTATION_REPO" >&2
            return 1
        fi
        release_version="$(ci_redline_testing_version_from_artifact "$artifact_name")"
    else
        release_version="${requested_version#v}"
        release_tag="v$release_version"
        release_json="$(gh api "repos/${CI_REDLINE_TESTING_ATTESTATION_REPO}/releases/tags/${release_tag}")"
        artifact_name="$(
            jq -r '
                .assets[]
                | .name
                | select(test("^redline-testing-[0-9A-Za-z.+-]+-linux-x86_64\\.tar\\.gz$"))
            ' <<<"$release_json" | head -n 1
        )"
        if [ -z "$artifact_name" ]; then
            printf 'redline-testing release %s is missing the Linux tarball asset\n' "$release_tag" >&2
            return 1
        fi
        if ! jq -e --arg artifact "$artifact_name" '
            .assets | any(.name == $artifact) and any(.name == ($artifact + ".sha256"))
        ' <<<"$release_json" >/dev/null; then
            printf 'redline-testing release %s is missing the checksum sidecar for %s\n' \
                "$release_tag" "$artifact_name" >&2
            return 1
        fi
        if [ "$(ci_redline_testing_version_from_artifact "$artifact_name")" != "$release_version" ]; then
            printf 'redline-testing release %s asset/version mismatch: %s\n' \
                "$release_tag" "$artifact_name" >&2
            return 1
        fi
    fi

    CI_REDLINE_TESTING_VERSION="$release_version"
    CI_REDLINE_TESTING_RELEASE_TAG="$release_tag"
    CI_REDLINE_TESTING_ARTIFACT="$artifact_name"
    CI_REDLINE_TESTING_BASE_URL="https://github.com/${CI_REDLINE_TESTING_ATTESTATION_REPO}/releases/download/${CI_REDLINE_TESTING_RELEASE_TAG}"
    CI_REDLINE_TESTING_URL="${CI_REDLINE_TESTING_BASE_URL}/${CI_REDLINE_TESTING_ARTIFACT}"
    CI_REDLINE_TESTING_SHA256_URL="${CI_REDLINE_TESTING_URL}.sha256"
    CI_REDLINE_TESTING_RELEASE_MANIFEST_URL="${CI_REDLINE_TESTING_BASE_URL}/release-manifest.json"
}

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

ci_assert_redline_testing_official_artifacts() {
    ci_assert_artifacts \
        target/redline-testing/all.jsonl \
        target/redline-testing/official-evidence.json \
        target/redline-testing/all-manifest.json \
        target/redline-testing/summary.json \
        target/redline-testing/ranked.csv \
        target/redline-testing/manifest.json \
        target/redline-testing/provenance.json \
        target/redline-testing/memory-summary.json \
        target/redline-testing/memory-ranked.csv \
        target/redline-testing/memory-manifest.json \
        target/redline-testing/memory-provenance.json \
        target/redline-testing/beyond-sqlite-summary.json \
        target/redline-testing/beyond-sqlite-ranked.csv \
        target/redline-testing/beyond-sqlite-manifest.json \
        target/redline-testing/beyond-sqlite-provenance.json \
        target/redline-testing/redline-testing-provenance.env
}

# Install the pinned RedlineDB release package and print the CLI path on stdout.
# Status lines go to stderr so callers can safely capture the returned path.
ci_install_redlinedb_release() {
    local install_root="${CI_REDLINEDB_RELEASE_INSTALL_ROOT:-$PWD/target/ci/redlinedb-release/${CI_REDLINEDB_RELEASE_TAG}-${CI_REDLINEDB_RELEASE_ARTIFACT}}"
    local tmp_dir
    tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/redlinedb-release.XXXXXX")"
    local release_url="${CI_REDLINEDB_RELEASE_OVERRIDE_URL:-$CI_REDLINEDB_RELEASE_URL}"
    local release_sha256_url="${CI_REDLINEDB_RELEASE_OVERRIDE_SHA256_URL:-$CI_REDLINEDB_RELEASE_SHA256_URL}"

    case "$release_url:$release_sha256_url" in
        file://*:file://*) ;;
        *)
            printf 'RedlineDB release smoke accepts physical local file inputs only\n' >&2
            rm -rf "$tmp_dir"
            return 1
            ;;
    esac
    cp "${release_url#file://}" "$tmp_dir/$CI_REDLINEDB_RELEASE_ASSET"
    cp "${release_sha256_url#file://}" "$tmp_dir/$CI_REDLINEDB_RELEASE_ASSET.sha256"
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
    printf 'RedlineDB release asset verified: %s\n' "$release_url" >&2
    printf 'RedlineDB installed: %s (%s)\n' "$install_root/bin/redlinedb" "$version_output" >&2
    rm -rf "$tmp_dir"
    printf '%s\n' "$install_root/bin/redlinedb"
}

# Install the locally built and hash-bound redline-testing source tree + binary.
# Activated when `CI_REDLINE_TESTING_LOCAL_BIN`
# is non-empty (the caller must also point `CI_REDLINE_TESTING_LOCAL_SOURCE` at a
# checkout that contains contracts/, corpus/, metadata/, schemas/, templates/).
#
# The family controller supplies these exact bytes from its automatically
# removed reviewed checkout. Network downloads and external checkouts are not
# release inputs.
#
# Prints the staged binary path on stdout; status lines go to stderr.
ci_install_redline_testing_local() {
    local local_bin="${CI_REDLINE_TESTING_LOCAL_BIN:?CI_REDLINE_TESTING_LOCAL_BIN is required}"
    local local_source="${CI_REDLINE_TESTING_LOCAL_SOURCE:?CI_REDLINE_TESTING_LOCAL_SOURCE is required}"
    if [ ! -x "$local_bin" ]; then
        printf 'redline-testing local-bin escape hatch: CI_REDLINE_TESTING_LOCAL_BIN is not executable: %s\n' \
            "$local_bin" >&2
        return 1
    fi
    if [ ! -d "$local_source" ]; then
        printf 'redline-testing local-bin escape hatch: CI_REDLINE_TESTING_LOCAL_SOURCE is not a directory: %s\n' \
            "$local_source" >&2
        return 1
    fi
    local required_dir
    for required_dir in contracts corpus metadata schemas templates; do
        if [ ! -d "$local_source/$required_dir" ]; then
            printf 'redline-testing local-bin escape hatch: CI_REDLINE_TESTING_LOCAL_SOURCE missing %s/: %s\n' \
                "$required_dir" "$local_source" >&2
            return 1
        fi
    done

    local local_bin_abs
    local_bin_abs="$(cd "$(dirname "$local_bin")" && pwd)/$(basename "$local_bin")"
    local local_source_abs
    local_source_abs="$(cd "$local_source" && pwd)"

    local version_output
    if ! version_output="$("$local_bin_abs" --version)"; then
        printf 'redline-testing local-bin escape hatch: --version failed: %s\n' \
            "$local_bin_abs" >&2
        return 1
    fi
    local version
    version="${version_output#redline-testing }"
    if [ -z "$version" ] || [ "$version" = "$version_output" ]; then
        printf 'redline-testing local-bin escape hatch: unable to parse version from --version output: %q\n' \
            "$version_output" >&2
        return 1
    fi

    local binary_sha256
    binary_sha256="$(sha256sum "$local_bin_abs" | awk '{ print $1 }')"
    local binary_sha256_prefix="${binary_sha256:0:12}"

    local release_tag="v$version"
    local artifact_name="redline-testing-$version-linux-x86_64"
    local install_root="${CI_REDLINE_TESTING_INSTALL_ROOT:-$PWD/target/ci/redline-testing/local-${binary_sha256_prefix}}"

    rm -rf "$install_root"
    mkdir -p "$install_root/bin"

    install -m 0755 "$local_bin_abs" "$install_root/bin/redline-testing"

    # Copy the immutable inputs so the staged release remains recursively
    # symlink-free and independent of the source checkout.
    local source_dir
    for source_dir in contracts corpus metadata schemas templates; do
        cp -R "$local_source_abs/$source_dir" "$install_root/$source_dir"
    done

    # Synthesize a release-manifest.json that satisfies the schema at
    # schemas/release-manifest.schema.json and the field checks performed by
    # ci_verify_redline_testing_manifest. The `source` field is non-standard
    # but lets downstream lanes detect a local-bin install at a glance.
    local manifest="$install_root/release-manifest.json"
    local release_commit
    if release_commit="$(git -C "$local_source_abs" rev-parse HEAD 2>/dev/null)"; then
        :
    else
        release_commit="local-bin-unknown"
    fi
    cat > "$manifest" <<EOF
{
  "name": "redline-testing",
  "version": "$version",
  "target": "linux-x86_64",
  "release_commit": "$release_commit",
  "release_tag": "$release_tag",
  "binary": "bin/redline-testing",
  "binary_sha256": "$binary_sha256",
  "artifact_hashes": {},
  "generated_by": "ops/ci/lib.sh:ci_install_redline_testing_local",
  "source": "local-bin",
  "local_bin_path": "$local_bin_abs",
  "local_source_path": "$local_source_abs"
}
EOF

    # Mirror the packaged-file sanity checks from the official path.
    local path
    for path in \
        corpus/sqlite_parity/generated_manifest.json \
        metadata/beyond_sqlite/features.json \
        schemas/raw-record.schema.json \
        schemas/release-manifest.schema.json \
        templates/README.sqlite-parity.md
    do
        if [ ! -s "$install_root/$path" ]; then
            printf 'redline-testing local-bin escape hatch: missing packaged file: %s\n' \
                "$install_root/$path" >&2
            return 1
        fi
    done

    # Re-run the version round-trip against the staged path so the rest of the
    # function only depends on install_root.
    if ! version_output="$("$install_root/bin/redline-testing" --version)"; then
        printf 'redline-testing local-bin escape hatch: staged --version failed: %s\n' \
            "$install_root/bin/redline-testing" >&2
        return 1
    fi
    if [ "$version_output" != "redline-testing $version" ]; then
        printf 'redline-testing local-bin escape hatch: version round-trip mismatch: expected %q, got %q\n' \
            "redline-testing $version" "$version_output" >&2
        return 1
    fi

    local manifest_sha256
    manifest_sha256="$(sha256sum "$manifest" | awk '{ print $1 }')"

    # Update the same globals the official path updates so downstream helpers
    # (load_redline_testing_provenance, the report gate, evidence consumers) see
    # a consistent view of the install.
    CI_REDLINE_TESTING_VERSION="$version"
    CI_REDLINE_TESTING_RELEASE_TAG="$release_tag"
    CI_REDLINE_TESTING_ARTIFACT="$artifact_name.tar.gz"
    CI_REDLINE_TESTING_BASE_URL="local-bin://${local_source_abs}"
    CI_REDLINE_TESTING_URL="local-bin://${local_bin_abs}"
    CI_REDLINE_TESTING_SHA256_URL="local-bin://${local_bin_abs}.sha256"
    CI_REDLINE_TESTING_RELEASE_MANIFEST_URL="local-bin://${install_root}/release-manifest.json"

    ci_verify_redline_testing_manifest "$install_root" "$binary_sha256"

    printf 'redline-testing local-bin escape hatch active: %s\n' "$local_bin_abs" >&2
    printf 'redline-testing local-bin source tree: %s\n' "$local_source_abs" >&2
    printf 'redline-testing local-bin binary sha256: %s\n' "$binary_sha256" >&2
    printf 'redline-testing installed: %s (%s)\n' \
        "$install_root/bin/redline-testing" "$version_output" >&2

    {
        printf 'CI_REDLINE_TESTING_REQUESTED_VERSION=%q\n' "${CI_REDLINE_TESTING_REQUESTED_VERSION:-$version}"
        printf 'CI_REDLINE_TESTING_VERSION=%q\n' "$CI_REDLINE_TESTING_VERSION"
        printf 'CI_REDLINE_TESTING_RELEASE_TAG=%q\n' "$CI_REDLINE_TESTING_RELEASE_TAG"
        printf 'CI_REDLINE_TESTING_ARTIFACT=%q\n' "$CI_REDLINE_TESTING_ARTIFACT"
        printf 'CI_REDLINE_TESTING_BASE_URL=%q\n' "$CI_REDLINE_TESTING_BASE_URL"
        printf 'CI_REDLINE_TESTING_URL=%q\n' "$CI_REDLINE_TESTING_URL"
        printf 'CI_REDLINE_TESTING_SHA256_URL=%q\n' "$CI_REDLINE_TESTING_SHA256_URL"
        printf 'CI_REDLINE_TESTING_RELEASE_MANIFEST_URL=%q\n' "$CI_REDLINE_TESTING_RELEASE_MANIFEST_URL"
        printf 'CI_REDLINE_TESTING_SHA256=%q\n' "$binary_sha256"
        printf 'CI_REDLINE_TESTING_RELEASE_TARBALL_SHA256=%q\n' "$binary_sha256"
        printf 'CI_REDLINE_TESTING_RELEASE_MANIFEST_PATH=%q\n' "release-manifest.json"
        printf 'CI_REDLINE_TESTING_RELEASE_MANIFEST_SHA256=%q\n' "$manifest_sha256"
        printf 'CI_REDLINE_TESTING_BIN_PATH=%q\n' "bin/redline-testing"
        printf 'CI_REDLINE_TESTING_BIN=%q\n' "$install_root/bin/redline-testing"
        printf 'CI_REDLINE_TESTING_BIN_SHA256=%q\n' "$binary_sha256"
        printf 'CI_REDLINE_TESTING_RELEASE_BINARY_SHA256=%q\n' "$binary_sha256"
        printf 'CI_REDLINE_TESTING_VERSION_OUTPUT=%q\n' "$version_output"
        printf 'CI_REDLINE_TESTING_SOURCE=%q\n' "local-bin"
        printf 'CI_REDLINE_TESTING_LOCAL_BIN=%q\n' "$local_bin_abs"
        printf 'CI_REDLINE_TESTING_LOCAL_SOURCE=%q\n' "$local_source_abs"
    } > "$install_root/redline-testing-provenance.env"

    printf '%s\n' "$install_root/bin/redline-testing"
}

ci_install_redline_testing() {
    if [ -z "${CI_REDLINE_TESTING_LOCAL_BIN:-}" ]; then
        printf 'CI_REDLINE_TESTING_LOCAL_BIN and CI_REDLINE_TESTING_LOCAL_SOURCE are required; network release resolution is forbidden\n' >&2
        return 1
    fi
    ci_install_redline_testing_local
    return $?
    # Historical network installer retained below as unreachable migration
    # context. No authoritative caller can enter it.
    ci_resolve_redline_testing_release
    local artifact_name="${CI_REDLINE_TESTING_ARTIFACT##*/}"
    local install_root="${CI_REDLINE_TESTING_INSTALL_ROOT:-$PWD/target/ci/redline-testing/${CI_REDLINE_TESTING_VERSION}-${artifact_name%.tar.gz}}"
    local tmp_dir
    tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/redline-testing-release.XXXXXX")"
    local extract_dir="$tmp_dir/extract"
    mkdir -p "$extract_dir"

    curl --fail --location --retry 5 --retry-all-errors --silent --show-error \
        -o "$tmp_dir/$artifact_name" "$CI_REDLINE_TESTING_URL"
    curl --fail --location --retry 5 --retry-all-errors --silent --show-error \
        -o "$tmp_dir/$artifact_name.sha256" "$CI_REDLINE_TESTING_SHA256_URL"

    local expected_sha256
    local actual_sha256
    expected_sha256="$(grep -Eo '[[:xdigit:]]{64}' "$tmp_dir/$artifact_name.sha256" | head -n 1 || true)"
    if [[ ! "$expected_sha256" =~ ^[[:xdigit:]]{64}$ ]]; then
        printf 'redline-testing checksum file did not contain a SHA256 digest: %s\n' \
            "$CI_REDLINE_TESTING_SHA256_URL" >&2
        rm -rf "$tmp_dir"
        return 1
    fi
    actual_sha256="$(sha256sum "$tmp_dir/$artifact_name" | awk '{ print $1 }')"
    if [ "$actual_sha256" != "$expected_sha256" ]; then
        printf 'redline-testing SHA256 mismatch for %s: expected %s, got %s\n' \
            "$CI_REDLINE_TESTING_URL" "$expected_sha256" "$actual_sha256" >&2
        rm -rf "$tmp_dir"
        return 1
    fi
    if [ -n "$CI_REDLINE_TESTING_EXPECTED_TARBALL_SHA256" ] && [ "$actual_sha256" != "$CI_REDLINE_TESTING_EXPECTED_TARBALL_SHA256" ]; then
        printf 'redline-testing pinned SHA256 mismatch for %s: pinned %s, got %s\n' \
            "$CI_REDLINE_TESTING_URL" "$CI_REDLINE_TESTING_EXPECTED_TARBALL_SHA256" "$actual_sha256" >&2
        rm -rf "$tmp_dir"
        return 1
    fi

    ci_verify_redline_testing_attestation "$tmp_dir/$artifact_name"

    tar -xzf "$tmp_dir/$artifact_name" -C "$extract_dir"

    local package_dir
    package_dir="$extract_dir/${artifact_name%.tar.gz}"
    if [ ! -x "$package_dir/bin/redline-testing" ]; then
        local redline_testing_bin
        redline_testing_bin="$(find "$extract_dir" -type f -path '*/bin/redline-testing' -perm -111 -print -quit)"
        if [ -z "$redline_testing_bin" ]; then
            printf 'redline-testing release asset missing executable: %s\n' \
                "$package_dir/bin/redline-testing" >&2
            rm -rf "$tmp_dir"
            return 1
        fi
        package_dir="${redline_testing_bin%/bin/redline-testing}"
    fi

    local manifest="$package_dir/release-manifest.json"
    if [ ! -s "$manifest" ]; then
        printf 'redline-testing release asset missing manifest: %s\n' "$manifest" >&2
        rm -rf "$tmp_dir"
        return 1
    fi
    for path in \
        corpus/sqlite_parity/generated_manifest.json \
        metadata/beyond_sqlite/features.json \
        schemas/raw-record.schema.json \
        schemas/release-manifest.schema.json \
        templates/README.sqlite-parity.md
    do
        if [ ! -s "$package_dir/$path" ]; then
            printf 'redline-testing release asset missing packaged file: %s\n' "$package_dir/$path" >&2
            rm -rf "$tmp_dir"
            return 1
        fi
    done
    local manifest_version
    local manifest_tag
    manifest_version="$(jq -r '.version // empty' "$manifest")"
    manifest_tag="$(jq -r '.release_tag // empty' "$manifest")"
    if [ -z "$manifest_version" ] || [ -z "$manifest_tag" ]; then
        printf 'redline-testing release manifest missing required version/tag fields: %s\n' "$manifest" >&2
        rm -rf "$tmp_dir"
        return 1
    fi
    if [ "$manifest_version" != "$CI_REDLINE_TESTING_VERSION" ]; then
        printf 'redline-testing manifest version mismatch: expected %s, got %s\n' \
            "$CI_REDLINE_TESTING_VERSION" "$manifest_version" >&2
        rm -rf "$tmp_dir"
        return 1
    fi
    if [ "$manifest_tag" != "$CI_REDLINE_TESTING_RELEASE_TAG" ]; then
        printf 'redline-testing manifest tag mismatch: expected %s, got %s\n' \
            "$CI_REDLINE_TESTING_RELEASE_TAG" "$manifest_tag" >&2
        rm -rf "$tmp_dir"
        return 1
    fi

    rm -rf "$install_root"
    mkdir -p "$install_root"
    cp -R "$package_dir/." "$install_root/"

    local version_output
    if ! version_output="$("$install_root/bin/redline-testing" --version)"; then
        printf 'redline-testing executable failed --version: %s\n' \
            "$install_root/bin/redline-testing" >&2
        rm -rf "$tmp_dir"
        return 1
    fi
    if [ "$version_output" != "redline-testing $CI_REDLINE_TESTING_VERSION" ]; then
        printf 'redline-testing version mismatch: expected %s, got %s\n' \
            "redline-testing $CI_REDLINE_TESTING_VERSION" "$version_output" >&2
        rm -rf "$tmp_dir"
        return 1
    fi
    local binary_sha256
    binary_sha256="$(sha256sum "$install_root/bin/redline-testing" | awk '{ print $1 }')"
    if [ -n "$CI_REDLINE_TESTING_EXPECTED_BINARY_SHA256" ] && [ "$binary_sha256" != "$CI_REDLINE_TESTING_EXPECTED_BINARY_SHA256" ]; then
        printf 'redline-testing binary SHA256 mismatch: pinned %s, got %s\n' \
            "$CI_REDLINE_TESTING_EXPECTED_BINARY_SHA256" "$binary_sha256" >&2
        rm -rf "$tmp_dir"
        return 1
    fi
    local manifest_sha256
    manifest_sha256="$(sha256sum "$install_root/release-manifest.json" | awk '{ print $1 }')"
    CI_REDLINE_TESTING_VERSION="$manifest_version"
    CI_REDLINE_TESTING_RELEASE_TAG="$manifest_tag"
    CI_REDLINE_TESTING_RELEASE_MANIFEST_URL="${CI_REDLINE_TESTING_RELEASE_MANIFEST_URL:-${CI_REDLINE_TESTING_BASE_URL}/release-manifest.json}"
    ci_verify_redline_testing_manifest "$install_root" "$binary_sha256"
    printf 'redline-testing release asset verified: %s\n' "$CI_REDLINE_TESTING_URL" >&2
    printf 'redline-testing release sha256: %s\n' "$actual_sha256" >&2
    printf 'redline-testing binary sha256: %s\n' "$binary_sha256" >&2
    printf 'redline-testing installed: %s (%s)\n' \
        "$install_root/bin/redline-testing" "$version_output" >&2
    {
        printf 'CI_REDLINE_TESTING_REQUESTED_VERSION=%q\n' "${CI_REDLINE_TESTING_REQUESTED_VERSION:-$CI_REDLINE_TESTING_VERSION}"
        printf 'CI_REDLINE_TESTING_VERSION=%q\n' "$CI_REDLINE_TESTING_VERSION"
        printf 'CI_REDLINE_TESTING_RELEASE_TAG=%q\n' "$CI_REDLINE_TESTING_RELEASE_TAG"
        printf 'CI_REDLINE_TESTING_ARTIFACT=%q\n' "$artifact_name"
        printf 'CI_REDLINE_TESTING_BASE_URL=%q\n' "$CI_REDLINE_TESTING_BASE_URL"
        printf 'CI_REDLINE_TESTING_URL=%q\n' "$CI_REDLINE_TESTING_URL"
        printf 'CI_REDLINE_TESTING_SHA256_URL=%q\n' "$CI_REDLINE_TESTING_SHA256_URL"
        printf 'CI_REDLINE_TESTING_RELEASE_MANIFEST_URL=%q\n' "$CI_REDLINE_TESTING_RELEASE_MANIFEST_URL"
        printf 'CI_REDLINE_TESTING_SHA256=%q\n' "$actual_sha256"
        printf 'CI_REDLINE_TESTING_RELEASE_TARBALL_SHA256=%q\n' "$actual_sha256"
        printf 'CI_REDLINE_TESTING_RELEASE_MANIFEST_PATH=%q\n' "release-manifest.json"
        printf 'CI_REDLINE_TESTING_RELEASE_MANIFEST_SHA256=%q\n' "$manifest_sha256"
        printf 'CI_REDLINE_TESTING_BIN_PATH=%q\n' "bin/redline-testing"
        printf 'CI_REDLINE_TESTING_BIN=%q\n' "$install_root/bin/redline-testing"
        printf 'CI_REDLINE_TESTING_BIN_SHA256=%q\n' "$binary_sha256"
        printf 'CI_REDLINE_TESTING_RELEASE_BINARY_SHA256=%q\n' "$binary_sha256"
        printf 'CI_REDLINE_TESTING_VERSION_OUTPUT=%q\n' "$version_output"
    } > "$install_root/redline-testing-provenance.env"
    rm -rf "$tmp_dir"
    printf '%s\n' "$install_root/bin/redline-testing"
}

ci_verify_redline_testing_manifest() {
    local install_root="${1:?install root required}"
    local binary_sha256="${2:?binary sha required}"
    local manifest="$install_root/release-manifest.json"
    if [ ! -s "$manifest" ]; then
        printf 'redline-testing release manifest missing: %s\n' "$manifest" >&2
        return 1
    fi
    grep -q '"name": "redline-testing"' "$manifest" || {
        printf 'redline-testing release manifest has wrong name: %s\n' "$manifest" >&2
        return 1
    }
    grep -q "\"version\": \"$CI_REDLINE_TESTING_VERSION\"" "$manifest" || {
        printf 'redline-testing release manifest has wrong version: %s\n' "$manifest" >&2
        return 1
    }
    grep -q "\"release_tag\": \"$CI_REDLINE_TESTING_RELEASE_TAG\"" "$manifest" || {
        printf 'redline-testing release manifest has wrong tag: %s\n' "$manifest" >&2
        return 1
    }
    grep -q "\"binary_sha256\": \"$binary_sha256\"" "$manifest" || {
        printf 'redline-testing release manifest binary hash mismatch: %s\n' "$manifest" >&2
        return 1
    }
}

ci_verify_redline_testing_attestation() {
    local artifact="${1:?artifact path required}"
    local receipt="${REDLINE_ORACLE_CUSTODY_RECEIPT:?REDLINE_ORACLE_CUSTODY_RECEIPT is required}"
    [ -f "$artifact" ] && [ ! -L "$artifact" ] || return 1
    [ -f "$receipt" ] && [ ! -L "$receipt" ] || return 1
    [ "$(stat -c %h "$artifact")" = 1 ] || return 1
    [ "$(stat -c %h "$receipt")" = 1 ] || return 1
    local artifact_sha256
    artifact_sha256="$(sha256sum "$artifact" | awk '{print $1}')"
    jq -e --arg artifact_sha256 "$artifact_sha256" '
        .schema_version == "redline.custody-receipt/v1"
        and .status == "pass"
        and (.artifact_sha256 | type == "string")
        and .artifact_sha256 == $artifact_sha256
    ' "$receipt" >/dev/null
}

ci_verify_redlinedb_release_smoke() {
    local redlinedb_bin
    local output
    ci_prepare_redlinedb_release_smoke
    redlinedb_bin="$(ci_install_redlinedb_release)"
    output="$(printf 'SELECT 1;\n' | "$redlinedb_bin" -batch -bail -list -separator '|' :memory:)"
    if [ "$output" != "1" ]; then
        printf 'RedlineDB release smoke failed: expected `1`, got `%s`\n' "$output" >&2
        return 1
    fi
    printf 'RedlineDB release smoke passed: %s\n' "$redlinedb_bin" >&2
}

ci_prepare_redlinedb_release_smoke() {
    if [ "${CI_REDLINEDB_RELEASE_BUILD_LOCAL:-1}" != "1" ]; then
        return 0
    fi
    case "${CI_REDLINEDB_RELEASE_URL:-}" in
        file://*)
            return 0
            ;;
    esac

    local source_dir="$PWD"
    local smoke_root="${CI_REDLINEDB_RELEASE_SMOKE_DIR:-$source_dir/target/ci/redlinedb-release-smoke}"
    local output_dir
    output_dir="$(mkdir -p "$smoke_root" && cd "$smoke_root" && pwd)"

    rm -rf "$output_dir/${CI_REDLINEDB_RELEASE_ASSET%.tar.gz}" \
        "$output_dir/$CI_REDLINEDB_RELEASE_ASSET" \
        "$output_dir/$CI_REDLINEDB_RELEASE_ASSET.sha256"

    TAG="$CI_REDLINEDB_RELEASE_TAG" \
    ARTIFACT="$CI_REDLINEDB_RELEASE_ARTIFACT" \
    LIB_NAME="libredlinedb.so" \
    TARGET="x86_64-unknown-linux-gnu" \
    SOURCE_DIR="$source_dir" \
    OUTPUT_DIR="$output_dir" \
        bash ops/ci/release-build.sh

    CI_REDLINEDB_RELEASE_OVERRIDE_URL="file://${output_dir}/${CI_REDLINEDB_RELEASE_ASSET}"
    CI_REDLINEDB_RELEASE_OVERRIDE_SHA256_URL="${CI_REDLINEDB_RELEASE_OVERRIDE_URL}.sha256"
    export CI_REDLINEDB_RELEASE_OVERRIDE_URL
    export CI_REDLINEDB_RELEASE_OVERRIDE_SHA256_URL
}

ci_install_gitleaks() {
    local binary
    local version_output
    binary="$(type -P gitleaks 2>/dev/null || true)"
    if [ -z "$binary" ] || [ ! -f "$binary" ] || [ ! -x "$binary" ] || [ -L "$binary" ]; then
        printf 'missing physical local gitleaks %s; network installation is forbidden\n' \
            "$CI_GITLEAKS_VERSION" >&2
        return 1
    fi
    version_output="$("$binary" version)"
    case "$version_output" in
        "$CI_GITLEAKS_VERSION"*) ;;
        *)
            printf 'installed gitleaks version mismatch: got %s, expected %s\n' \
                "$version_output" "$CI_GITLEAKS_VERSION" >&2
            return 1
            ;;
    esac
    printf 'physical local gitleaks verified: %s (%s)\n' "$binary" "$version_output"
}

# Compatibility name retained for existing Core lane dispatchers. This no
# longer installs or fetches anything; it only verifies the governed binary.
ci_install_jankurai() {
    require_jankurai
    printf 'governed Jankurai verified: %s version=%s sha256=%s\n' \
        "$JERYU_GOVERNED_JANKURAI_BIN" "$JERYU_JANKURAI_VERSION" "$JERYU_JANKURAI_SHA256"
}

ci_install_jankurai_logged() {
    local log_path="$1"
    mkdir -p "$(dirname "$log_path")"

    if ! ci_install_jankurai >"$log_path" 2>&1; then
        cat "$log_path" >&2
        return 1
    fi

    cat "$log_path"
}
