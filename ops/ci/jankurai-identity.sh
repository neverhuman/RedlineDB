#!/usr/bin/env bash
# Exact governed Jankurai identity shared by every Redline audit entrypoint.

if [[ -n "${REDLINE_JANKURAI_IDENTITY_LOADED:-}" ]]; then
    return 0
fi
readonly REDLINE_JANKURAI_IDENTITY_LOADED=1

readonly REDLINE_JANKURAI_BIN="/home/ubuntu/.jeryu/bin/jankurai"
readonly REDLINE_JANKURAI_VERSION="jankurai 1.6.11"
readonly REDLINE_JANKURAI_REPO="http://127.0.0.1:8787/git/jeryu/jankurai.git"
readonly REDLINE_JANKURAI_REV="dface7397fe24d46b0b1885ddd5782c34edbff49"
readonly REDLINE_JANKURAI_TAG="v1.6.11-deadlang-precision-split.1"
readonly REDLINE_JANKURAI_TREE="34a8a1fb59bc4ebfadf12c45d95f169d06acc781"
readonly REDLINE_JANKURAI_ARCHIVE_SHA256="2fbca5d04083e3c8d32f383d5b6b4520b8911690b26968c6fbcb210e1202b938"
readonly REDLINE_JANKURAI_CARGO_LOCK_SHA256="b9acb981c326226a687d0b6703e4f7ee303148e9e1a6dda1aa03d77988820f6a"
readonly REDLINE_JANKURAI_BINARY_SHA256="fdb42e5fa7d9851c0729e59bf1e582c895aa9cfc03a7175b420c6025d2fd014e"
readonly REDLINE_JANKURAI_RUSTC="rustc 1.95.0 (59807616e 2026-04-14)"
readonly REDLINE_JANKURAI_CARGO="cargo 1.95.0 (f2d3ce0bd 2026-03-21)"
readonly REDLINE_JANKURAI_TARGET="x86_64-unknown-linux-gnu"
readonly REDLINE_JANKURAI_BUILD_MODE="cargo-install-locked-offline-path-v1"
readonly REDLINE_JANKURAI_RECEIPT_DIR="/home/ubuntu/.jeryu/receipts/jankurai/sha256"
readonly REDLINE_JANKURAI_MANIFEST_REPO="http://127.0.0.1:8787/git/jeryu/jeryu-tool.git"
readonly REDLINE_JANKURAI_PROTECTION_POLICY="immutable-main-v1"
readonly REDLINE_JANKURAI_NETWORK_SCOPE="local-forge-source-plus-offline-cargo"
readonly REDLINE_JANKURAI_NO_PROXY="127.0.0.1,localhost,::1"

export JANKURAI_BIN="${REDLINE_JANKURAI_BIN}"
export JANKURAI_NO_UPDATE_CHECK=1
export GIT_TERMINAL_PROMPT=0

require_governed_jankurai() {
    local actual_sha actual_version normalized receipt receipt_digest receipt_sha tool
    local found_receipt=0

    for tool in jq realpath sha256sum; do
        command -v "${tool}" >/dev/null 2>&1 || {
            printf 'required governed-jankurai verifier tool is unavailable: %s\n' "${tool}" >&2
            return 1
        }
    done
    if [[ ! -f "${REDLINE_JANKURAI_BIN}" || -L "${REDLINE_JANKURAI_BIN}" || ! -x "${REDLINE_JANKURAI_BIN}" ]]; then
        printf 'governed jankurai must be an executable regular file: %s\n' "${REDLINE_JANKURAI_BIN}" >&2
        return 1
    fi
    normalized="$(realpath -m "${REDLINE_JANKURAI_BIN}")"
    if [[ "${normalized}" != "${REDLINE_JANKURAI_BIN}" ]]; then
        printf 'governed jankurai path traverses a symlink: %s -> %s\n' \
            "${REDLINE_JANKURAI_BIN}" "${normalized}" >&2
        return 1
    fi

    # Hash before execution so a wrong host binary is never invoked.
    actual_sha="$(sha256sum "${REDLINE_JANKURAI_BIN}" | awk '{print $1}')"
    if [[ "${actual_sha}" != "${REDLINE_JANKURAI_BINARY_SHA256}" ]]; then
        printf 'governed jankurai digest mismatch: expected=%s actual=%s path=%s\n' \
            "${REDLINE_JANKURAI_BINARY_SHA256}" "${actual_sha}" "${REDLINE_JANKURAI_BIN}" >&2
        return 1
    fi

    for receipt in "${REDLINE_JANKURAI_RECEIPT_DIR}"/*.json; do
        [[ -f "${receipt}" ]] || continue
        receipt_digest="$(basename "${receipt}" .json)"
        [[ "${receipt_digest}" =~ ^[0-9a-f]{64}$ ]] || continue
        receipt_sha="$(sha256sum "${receipt}" | awk '{print $1}')"
        [[ "${receipt_sha}" == "${receipt_digest}" ]] || continue
        if jq -e \
            --arg remote "${REDLINE_JANKURAI_REPO}" \
            --arg commit "${REDLINE_JANKURAI_REV}" \
            --arg tag "${REDLINE_JANKURAI_TAG}" \
            --arg tree "${REDLINE_JANKURAI_TREE}" \
            --arg archive "${REDLINE_JANKURAI_ARCHIVE_SHA256}" \
            --arg lock "${REDLINE_JANKURAI_CARGO_LOCK_SHA256}" \
            --arg rustc "${REDLINE_JANKURAI_RUSTC}" \
            --arg cargo "${REDLINE_JANKURAI_CARGO}" \
            --arg target "${REDLINE_JANKURAI_TARGET}" \
            --arg mode "${REDLINE_JANKURAI_BUILD_MODE}" \
            --arg digest "${REDLINE_JANKURAI_BINARY_SHA256}" \
            --arg version "${REDLINE_JANKURAI_VERSION}" \
            --arg path "${REDLINE_JANKURAI_BIN}" \
            --arg manifest_repo "${REDLINE_JANKURAI_MANIFEST_REPO}" \
            --arg protection "${REDLINE_JANKURAI_PROTECTION_POLICY}" \
            --arg network_scope "${REDLINE_JANKURAI_NETWORK_SCOPE}" \
            --arg no_proxy "${REDLINE_JANKURAI_NO_PROXY}" \
            '.schema == "jeryu.jankurai-installation/v1" and
             .source.remote == $remote and .source.commit == $commit and .source.tag == $tag and
             .source.tree == $tree and .source.archive_sha256 == $archive and
             .source.cargo_lock_sha256 == $lock and .source.verification == "release-authoritative" and
             .build.rustc == $rustc and .build.cargo == $cargo and
             .build.target_triple == $target and .build.mode == $mode and
             .build.cargo_net_offline == true and .build.dedicated_cargo_home == true and
             .build.git_global_config_disabled == true and .build.git_system_config_disabled == true and
             .build.git_http_follow_redirects == false and .build.git_terminal_prompt == false and
             .build.jankurai_update_check == false and
             .build.network_scope == $network_scope and .build.no_proxy == $no_proxy and
             .governance.status == "governed" and
             .governance.manifest_repo == $manifest_repo and
             (.governance.manifest_commit | test("^[0-9a-f]{40}$")) and
             (.governance.manifest_tree | test("^[0-9a-f]{40}$")) and
             (.governance.manifest_sha256 | test("^[0-9a-f]{64}$")) and
             .governance.protected_main == true and
             .governance.protection_policy == $protection and
             .binary.sha256 == $digest and .binary.version_output == $version and
             .installation.path == $path and .installation.atomic == true and
             .test_mode == false and .conclusion == "success"' \
            "${receipt}" >/dev/null; then
            export REDLINE_JANKURAI_RECEIPT="${receipt}"
            export REDLINE_JANKURAI_RECEIPT_SHA256="${receipt_digest}"
            found_receipt=1
            break
        fi
    done
    if [[ "${found_receipt}" -ne 1 ]]; then
        printf 'no content-addressed governed Jankurai receipt matches %s@%s\n' \
            "${REDLINE_JANKURAI_REPO}" "${REDLINE_JANKURAI_REV}" >&2
        return 1
    fi

    actual_version="$("${REDLINE_JANKURAI_BIN}" --version 2>/dev/null || true)"
    if [[ "${actual_version}" != "${REDLINE_JANKURAI_VERSION}" ]]; then
        printf 'governed jankurai version mismatch: expected=%s actual=%s\n' \
            "${REDLINE_JANKURAI_VERSION}" "${actual_version:-missing}" >&2
        return 1
    fi
}

run_governed_jankurai() {
    require_governed_jankurai || return 1
    "${REDLINE_JANKURAI_BIN}" "$@"
}
