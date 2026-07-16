#!/usr/bin/env bash
# Host-independent adversarial tests for the governed Jankurai verifier.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source_identity="$repo_root/ops/ci/jankurai-identity.sh"
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/redline-jankurai-identity.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT

fixture_bin="$tmp_dir/governed/jankurai"
receipt_dir="$tmp_dir/receipts"
governed_log="$tmp_dir/governed-executions.log"
shadow_log="$tmp_dir/shadow-executions.log"
mkdir -p "$(dirname "$fixture_bin")" "$receipt_dir" "$tmp_dir/shadow"

cat >"$fixture_bin" <<'FAKE'
#!/usr/bin/env bash
printf 'governed\n' >>"${FAKE_JANKURAI_EXEC_LOG:?}"
if [[ "${1:-}" == "--version" ]]; then
    printf 'jankurai 1.6.11\n'
fi
FAKE
chmod 0755 "$fixture_bin"
fixture_digest="$(sha256sum "$fixture_bin" | awk '{print $1}')"

cat >"$tmp_dir/shadow/jankurai" <<'SHADOW'
#!/usr/bin/env bash
printf 'shadow\n' >>"${SHADOW_JANKURAI_EXEC_LOG:?}"
exit 99
SHADOW
chmod 0755 "$tmp_dir/shadow/jankurai"

receipt_tmp="$tmp_dir/receipt.json"
jq -n \
    --arg path "$fixture_bin" \
    --arg digest "$fixture_digest" \
    '{
      schema: "jeryu.jankurai-installation/v1",
      source: {
        remote: "http://127.0.0.1:8787/git/jeryu/jankurai.git",
        commit: "dface7397fe24d46b0b1885ddd5782c34edbff49",
        tag: "v1.6.11-deadlang-precision-split.1",
        tree: "34a8a1fb59bc4ebfadf12c45d95f169d06acc781",
        archive_sha256: "2fbca5d04083e3c8d32f383d5b6b4520b8911690b26968c6fbcb210e1202b938",
        cargo_lock_sha256: "b9acb981c326226a687d0b6703e4f7ee303148e9e1a6dda1aa03d77988820f6a",
        verification: "release-authoritative"
      },
      build: {
        rustc: "rustc 1.95.0 (59807616e 2026-04-14)",
        cargo: "cargo 1.95.0 (f2d3ce0bd 2026-03-21)",
        target_triple: "x86_64-unknown-linux-gnu",
        mode: "cargo-install-locked-offline-path-v1",
        cargo_net_offline: true,
        dedicated_cargo_home: true,
        git_global_config_disabled: true,
        git_system_config_disabled: true,
        git_http_follow_redirects: false,
        git_terminal_prompt: false,
        jankurai_update_check: false,
        network_scope: "local-forge-source-plus-offline-cargo",
        no_proxy: "127.0.0.1,localhost,::1"
      },
      governance: {
        status: "governed",
        manifest_repo: "http://127.0.0.1:8787/git/jeryu/jeryu-tool.git",
        manifest_commit: "1111111111111111111111111111111111111111",
        manifest_tree: "2222222222222222222222222222222222222222",
        manifest_sha256: "3333333333333333333333333333333333333333333333333333333333333333",
        protected_main: true,
        protection_policy: "immutable-main-v1"
      },
      binary: {sha256: $digest, version_output: "jankurai 1.6.11"},
      installation: {path: $path, atomic: true},
      test_mode: false,
      conclusion: "success"
    }' >"$receipt_tmp"
receipt_sha="$(sha256sum "$receipt_tmp" | awk '{print $1}')"
receipt_path="$receipt_dir/$receipt_sha.json"
mv "$receipt_tmp" "$receipt_path"

fixture_identity="$tmp_dir/jankurai-identity.sh"
sed \
    -e "s|^readonly REDLINE_JANKURAI_BIN=.*|readonly REDLINE_JANKURAI_BIN=\"$fixture_bin\"|" \
    -e "s|^readonly REDLINE_JANKURAI_BINARY_SHA256=.*|readonly REDLINE_JANKURAI_BINARY_SHA256=\"$fixture_digest\"|" \
    -e "s|^readonly REDLINE_JANKURAI_RECEIPT_DIR=.*|readonly REDLINE_JANKURAI_RECEIPT_DIR=\"$receipt_dir\"|" \
    "$source_identity" >"$fixture_identity"

# A PATH and environment shadow must never be selected. The governed fixture
# executes twice: once for identity readback and once for the requested call.
PATH="$tmp_dir/shadow:$PATH" \
JANKURAI_BIN="$tmp_dir/shadow/jankurai" \
FAKE_JANKURAI_EXEC_LOG="$governed_log" \
SHADOW_JANKURAI_EXEC_LOG="$shadow_log" \
    bash -c '. "$1"; run_governed_jankurai --version >/dev/null' _ "$fixture_identity"
[[ "$(wc -l <"$governed_log")" -eq 2 ]]
[[ ! -e "$shadow_log" ]]

# A wrong digest is rejected before the candidate binary can execute.
: >"$governed_log"
printf '\n# tampered\n' >>"$fixture_bin"
if FAKE_JANKURAI_EXEC_LOG="$governed_log" \
    bash -c '. "$1"; run_governed_jankurai --version' _ "$fixture_identity" \
    >"$tmp_dir/tamper.out" 2>"$tmp_dir/tamper.err"; then
    printf 'tampered governed binary was accepted\n' >&2
    exit 1
fi
[[ ! -s "$governed_log" ]]
grep -F 'governed jankurai digest mismatch' "$tmp_dir/tamper.err" >/dev/null

# Restore the exact binary, then corrupt the content-addressed receipt. Receipt
# rejection must also happen before version readback executes the binary.
sed -i '$d' "$fixture_bin"
sed -i '$d' "$fixture_bin"
[[ "$(sha256sum "$fixture_bin" | awk '{print $1}')" == "$fixture_digest" ]]
: >"$governed_log"
printf '\n' >>"$receipt_path"
if FAKE_JANKURAI_EXEC_LOG="$governed_log" \
    bash -c '. "$1"; run_governed_jankurai --version' _ "$fixture_identity" \
    >"$tmp_dir/receipt.out" 2>"$tmp_dir/receipt.err"; then
    printf 'corrupt governed receipt was accepted\n' >&2
    exit 1
fi
[[ ! -s "$governed_log" ]]
grep -F 'no content-addressed governed Jankurai receipt matches' "$tmp_dir/receipt.err" >/dev/null

# A policy-invalid receipt remains invalid even when its filename is a correct
# content address. Governance must be checked before the binary is executed.
wrong_governance_tmp="$tmp_dir/wrong-governance.json"
jq '.governance.protected_main = false' "$receipt_path" >"$wrong_governance_tmp"
wrong_governance_sha="$(sha256sum "$wrong_governance_tmp" | awk '{print $1}')"
rm "$receipt_path"
wrong_governance_path="$receipt_dir/$wrong_governance_sha.json"
mv "$wrong_governance_tmp" "$wrong_governance_path"
: >"$governed_log"
if FAKE_JANKURAI_EXEC_LOG="$governed_log" \
    bash -c '. "$1"; run_governed_jankurai --version' _ "$fixture_identity" \
    >"$tmp_dir/governance.out" 2>"$tmp_dir/governance.err"; then
    printf 'receipt without protected-main governance was accepted\n' >&2
    exit 1
fi
[[ ! -s "$governed_log" ]]
grep -F 'no content-addressed governed Jankurai receipt matches' "$tmp_dir/governance.err" >/dev/null

# Static policy: consumer selection cannot regress to PATH, user-local paths,
# an environment override, or a repository-local Cargo install.
if rg -n 'command -v jankurai|JANKURAI_BIN:-|JANKURAI_FALLBACK_BIN|\.cargo/bin/jankurai|\.local/bin/jankurai|cargo install.*jankurai' \
    "$source_identity" "$repo_root/ops/ci/run-jankurai.sh"; then
    printf 'forbidden Jankurai selection or installation pattern found\n' >&2
    exit 1
fi

printf 'governed Jankurai identity adversarial tests: pass\n'
