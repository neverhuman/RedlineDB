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
readonly CI_GITLEAKS_ASSET="${CI_GITLEAKS_ASSET:-gitleaks_${CI_GITLEAKS_VERSION}_linux_x64.tar.gz}"
readonly CI_GITLEAKS_RELEASE_BASE_URL="${CI_GITLEAKS_RELEASE_BASE_URL:-https://github.com/gitleaks/gitleaks/releases/download/v${CI_GITLEAKS_VERSION}}"
readonly CI_GITLEAKS_ASSET_URL="${CI_GITLEAKS_ASSET_URL:-${CI_GITLEAKS_RELEASE_BASE_URL}/${CI_GITLEAKS_ASSET}}"
readonly CI_GITLEAKS_CHECKSUMS_URL="${CI_GITLEAKS_CHECKSUMS_URL:-${CI_GITLEAKS_RELEASE_BASE_URL}/gitleaks_${CI_GITLEAKS_VERSION}_checksums.txt}"
readonly CI_REDLINEDB_RELEASE_TAG="${CI_REDLINEDB_RELEASE_TAG:-v2.0.6}"
readonly CI_REDLINEDB_RELEASE_ARTIFACT="${CI_REDLINEDB_RELEASE_ARTIFACT:-linux-x86_64}"
readonly CI_REDLINEDB_RELEASE_ASSET="${CI_REDLINEDB_RELEASE_ASSET:-redlinedb-${CI_REDLINEDB_RELEASE_TAG}-${CI_REDLINEDB_RELEASE_ARTIFACT}.tar.gz}"
readonly CI_REDLINEDB_RELEASE_BASE_URL="${CI_REDLINEDB_RELEASE_BASE_URL:-https://github.com/neverhuman/RedlineDB/releases/download/${CI_REDLINEDB_RELEASE_TAG}}"
readonly CI_REDLINEDB_RELEASE_URL="${CI_REDLINEDB_RELEASE_URL:-${CI_REDLINEDB_RELEASE_BASE_URL}/${CI_REDLINEDB_RELEASE_ASSET}}"
readonly CI_REDLINEDB_RELEASE_SHA256_URL="${CI_REDLINEDB_RELEASE_SHA256_URL:-${CI_REDLINEDB_RELEASE_URL}.sha256}"
CI_REDLINE_TESTING_VERSION="${CI_REDLINE_TESTING_VERSION:-latest}"
CI_REDLINE_TESTING_EXPECTED_TARBALL_SHA256="${CI_REDLINE_TESTING_EXPECTED_TARBALL_SHA256:-}"
CI_REDLINE_TESTING_EXPECTED_BINARY_SHA256="${CI_REDLINE_TESTING_EXPECTED_BINARY_SHA256:-}"
readonly CI_REDLINE_TESTING_ATTESTATION_REPO="${CI_REDLINE_TESTING_ATTESTATION_REPO:-neverhuman/redline-testing}"
readonly CI_JANKURAI_VERSION="1.6.11"
readonly CI_JANKURAI_SHA256="96d99e6e7d8dc9cf23df1081edd1f975231456592f81d9405385219a2c7298aa"

# The release sandbox owns PATH. Freeze its selected executable before the
# wrapper below shadows the command name; every use still validates the exact
# physical path, version, and digest.
CI_JANKURAI_BIN="$(type -P jankurai 2>/dev/null || true)"
readonly CI_JANKURAI_BIN

# Keep literal `jankurai` commands visible to the adoption auditor without
# allowing a later PATH change to select different bytes.
jankurai() {
    ci_validate_jankurai_binary \
        "$CI_JANKURAI_BIN" \
        "$CI_JANKURAI_VERSION" \
        "$CI_JANKURAI_SHA256" || return 1
    "$CI_JANKURAI_BIN" "$@"
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

    curl -fsSL -o "$tmp_dir/$CI_REDLINEDB_RELEASE_ASSET" "$release_url"
    curl -fsSL -o "$tmp_dir/$CI_REDLINEDB_RELEASE_ASSET.sha256" "$release_sha256_url"
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

# Install a locally-built redline-testing source tree + binary, mimicking the
# layout produced by a tagged release. Activated when `CI_REDLINE_TESTING_LOCAL_BIN`
# is non-empty (the caller must also point `CI_REDLINE_TESTING_LOCAL_SOURCE` at a
# checkout that contains corpus/, metadata/, schemas/, templates/).
#
# This is an escape hatch for the `just redline-testing-official` lane so it can
# drive against unreleased redline-testing changes (e.g. a new corpus that has
# not yet been tagged on GitHub). It skips the URL download, sha256-against-URL,
# attestation, and manifest-version-against-pinned gates that the official path
# enforces; everything else (packaged-file checks, manifest schema fields,
# binary --version round-trip, provenance.env sidecar) still runs.
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
    for required_dir in corpus metadata schemas templates; do
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

    # Symlink the binary; sha256sum dereferences symlinks so downstream hashing
    # works against the underlying file.
    ln -s "$local_bin_abs" "$install_root/bin/redline-testing"

    # Symlink the four required top-level directories from the source tree. The
    # packaged-file sanity checks below only need to be able to stat() each
    # required path under install_root; the actual corpus data is `include_str!`d
    # into the binary at build time.
    local source_dir
    for source_dir in corpus metadata schemas templates; do
        ln -s "$local_source_abs/$source_dir" "$install_root/$source_dir"
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
    if [ -n "${CI_REDLINE_TESTING_LOCAL_BIN:-}" ]; then
        ci_install_redline_testing_local
        return $?
    fi
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
    case "$CI_REDLINE_TESTING_URL" in
        https://github.com/neverhuman/redline-testing/releases/download/*) ;;
        *)
            if [ "${CI_REDLINE_TESTING_REQUIRE_ATTESTATION:-0}" = "1" ]; then
                printf 'redline-testing attestation required for non-GitHub URL: %s\n' \
                    "$CI_REDLINE_TESTING_URL" >&2
                return 1
            fi
            printf 'redline-testing attestation skipped for local/non-GitHub URL: %s\n' \
                "$CI_REDLINE_TESTING_URL" >&2
            return 0
            ;;
    esac
    if ! command -v gh >/dev/null 2>&1; then
        printf 'gh is required to verify redline-testing artifact attestation\n' >&2
        return 127
    fi
    gh attestation verify "$artifact" --repo "$CI_REDLINE_TESTING_ATTESTATION_REPO" >/dev/null
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
    local install_dir
    install_dir="${CARGO_HOME:-$HOME/.cargo}/bin"
    mkdir -p "$install_dir"
    export PATH="$install_dir:$PATH"
    if [ -n "${GITHUB_PATH:-}" ]; then
        printf '%s\n' "$install_dir" >> "$GITHUB_PATH"
    fi

    local tmp_dir
    tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/gitleaks-release.XXXXXX")"

    curl --fail --location --retry 5 --retry-all-errors --silent --show-error \
        -o "$tmp_dir/$CI_GITLEAKS_ASSET" "$CI_GITLEAKS_ASSET_URL"
    curl --fail --location --retry 5 --retry-all-errors --silent --show-error \
        -o "$tmp_dir/gitleaks-checksums.txt" "$CI_GITLEAKS_CHECKSUMS_URL"
    grep "  ${CI_GITLEAKS_ASSET}$" "$tmp_dir/gitleaks-checksums.txt" \
        > "$tmp_dir/$CI_GITLEAKS_ASSET.sha256"
    (
        cd "$tmp_dir"
        sha256sum -c "$CI_GITLEAKS_ASSET.sha256"
    )

    tar -xzf "$tmp_dir/$CI_GITLEAKS_ASSET" -C "$tmp_dir" gitleaks
    install -m 0755 "$tmp_dir/gitleaks" "$install_dir/gitleaks"
    hash -r 2>/dev/null || true

    local version_output
    version_output="$(gitleaks version)"
    case "$version_output" in
        "$CI_GITLEAKS_VERSION"*) ;;
        *)
            printf 'installed gitleaks version mismatch: got %s, expected %s\n' \
                "$version_output" "$CI_GITLEAKS_VERSION" >&2
            return 1
            ;;
    esac
    printf 'gitleaks release asset verified: %s\n' "$CI_GITLEAKS_ASSET_URL"
    printf 'gitleaks installed: %s (%s)\n' "$(command -v gitleaks)" "$version_output"
    rm -rf "$tmp_dir"
}

# Validate arbitrary bytes for hostile tests. Production callers use only the
# sandbox-selected path frozen above and the fixed identity constants.
ci_validate_jankurai_binary() {
    local binary="${1:?jankurai binary path required}"
    local expected_version="${2:?jankurai version required}"
    local expected_sha256="${3:?jankurai sha256 required}"
    local actual_sha256 actual_version link_count

    command -v realpath >/dev/null 2>&1 || {
        printf 'missing required tool: realpath\n' >&2
        return 1
    }
    command -v sha256sum >/dev/null 2>&1 || {
        printf 'missing required tool: sha256sum\n' >&2
        return 1
    }
    command -v stat >/dev/null 2>&1 || {
        printf 'missing required tool: stat\n' >&2
        return 1
    }
    if [ ! -f "$binary" ] || [ ! -x "$binary" ] || [ -L "$binary" ]; then
        printf 'governed Jankurai path is missing, non-executable, non-regular, or symlinked: %s\n' \
            "$binary" >&2
        return 1
    fi
    [ "$(realpath -e "$binary")" = "$binary" ] || {
        printf 'governed Jankurai path contains a symlink or is not absolute: %s\n' \
            "$binary" >&2
        return 1
    }
    link_count="$(stat -c '%h' "$binary")"
    [ "$link_count" = 1 ] || {
        printf 'governed Jankurai path has unexpected hard links: %s count=%s\n' \
            "$binary" "$link_count" >&2
        return 1
    }
    actual_sha256="$(sha256sum "$binary" | awk '{print $1}')"
    [ "$actual_sha256" = "$expected_sha256" ] || {
        printf 'governed Jankurai digest mismatch: expected %s, got %s\n' \
            "$expected_sha256" "$actual_sha256" >&2
        return 1
    }
    actual_version="$("$binary" --version 2>/dev/null || true)"
    [ "$actual_version" = "jankurai $expected_version" ] || {
        printf 'governed Jankurai version mismatch: expected %s, got %s\n' \
            "jankurai $expected_version" "${actual_version:-no output}" >&2
        return 1
    }
}

ci_require_governed_jankurai() {
    ci_validate_jankurai_binary \
        "$CI_JANKURAI_BIN" \
        "$CI_JANKURAI_VERSION" \
        "$CI_JANKURAI_SHA256"
    printf 'governed Jankurai verified: %s version=%s sha256=%s\n' \
        "$CI_JANKURAI_BIN" "$CI_JANKURAI_VERSION" "$CI_JANKURAI_SHA256"
}

# Compatibility name retained for existing Core lane dispatchers. This no
# longer installs or fetches anything; it only verifies the governed binary.
ci_install_jankurai() {
    ci_require_governed_jankurai
}

ci_install_jankurai_logged() {
    local log_path="$1"
    mkdir -p "$(dirname "$log_path")"

    if ! ci_require_governed_jankurai >"$log_path" 2>&1; then
        cat "$log_path" >&2
        return 1
    fi

    cat "$log_path"
}
