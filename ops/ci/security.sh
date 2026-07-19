#!/usr/bin/env bash
# Fully local, pinned, fail-closed supply-chain evidence.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"

readonly CARGO_AUDIT_SHA256="1a17ff4c0449d1924aacda8dd20c06dccc3cceeed4dd17a71523f672bf97b70b"
readonly CARGO_DENY_SHA256="ef27c757f50d77c5c2d9114fbc6ad45d2b8903506cead473a70b8ee659ea7a18"
readonly GITLEAKS_SHA256="50b742abd7daad8bbddb6301f3017efb680632d9a5b3b4d8f137b3aac250e359"
readonly ZIZMOR_SHA256="6eabb307e2c0c35aa3b397d35518db66106c6909f86e7e0d7d898bc6587aa7fc"
readonly SYFT_SHA256="eb9714fb8e4b8f2a647e7bb312f1e0b9f83a7aa30418658bf46583cfa83d27d2"
readonly SYFT_CONFIG="${ROOT_DIR}/ops/ci/syft.yaml"
readonly RUSTSEC_DB_COMMIT="6e3286f4efa8c142fb33e5ea4342c8db6693cf34"
readonly RUSTSEC_DB_TREE="d12220aff0053a035739bec6e64aefbaafbf01a3"
readonly EXPECTED_SECURITY_COMMANDS="gitleaks detect; cargo audit; cargo deny; npm audit; zizmor; syft"

cargo_deny_home=""
cleanup_cargo_deny_home() {
  local rc=$?
  if [[ -n "$cargo_deny_home" && -d "$cargo_deny_home" ]]; then
    if [[ -n "$(find "$cargo_deny_home" -type l -print -quit)" ]]; then
      warn "preserving isolated cargo-deny home containing a symlink: $cargo_deny_home"
      return "$rc"
    fi
    rm -rf -- "$cargo_deny_home"
  fi
  return "$rc"
}
trap cleanup_cargo_deny_home EXIT

[[ "${REDLINE_SECURITY_COMMANDS:-$EXPECTED_SECURITY_COMMANDS}" == "$EXPECTED_SECURITY_COMMANDS" ]] \
  || fail "security command manifest does not match the governed release lane"

for tool in git jq npm sha256sum realpath tar; do
  has "$tool" || fail "required local security tool is missing: $tool"
done
[[ -z "$(git status --porcelain)" ]] || fail "security evidence requires a clean checkout"
cargo_audit_bin="${JAIN_REAL_CARGO_AUDIT:-}"
if [[ -n "$cargo_audit_bin" ]]; then
  jain_verify_exact_executable cargo-audit "$cargo_audit_bin" "$CARGO_AUDIT_SHA256" || exit 1
else
  cargo_audit_bin="$(jain_resolve_exact_executable cargo-audit cargo-audit "$CARGO_AUDIT_SHA256")" \
    || fail "governed cargo-audit is unavailable"
fi
cargo_deny_bin="${JAIN_REAL_CARGO_DENY:-}"
if [[ -n "$cargo_deny_bin" ]]; then
  jain_verify_exact_executable cargo-deny "$cargo_deny_bin" "$CARGO_DENY_SHA256" || exit 1
else
  cargo_deny_bin="$(jain_resolve_exact_executable cargo-deny cargo-deny "$CARGO_DENY_SHA256")" \
    || fail "governed cargo-deny is unavailable"
fi
gitleaks_bin="$(jain_resolve_exact_executable gitleaks gitleaks "$GITLEAKS_SHA256")" \
  || fail "governed gitleaks is unavailable"
zizmor_bin="$(jain_resolve_exact_executable zizmor zizmor "$ZIZMOR_SHA256")" \
  || fail "governed zizmor is unavailable"
syft_bin="$(jain_resolve_exact_executable syft syft "$SYFT_SHA256")" \
  || fail "governed Syft is unavailable"
[[ -f "$SYFT_CONFIG" && ! -L "$SYFT_CONFIG" ]] \
  || fail "pinned local Syft configuration is missing or symlinked"

rustsec_db="$(jain_governed_advisory_db_path "$RUSTSEC_DB_COMMIT")" \
  || fail "fleet-governed RustSec database selection failed"
jain_verify_governed_advisory_db \
  "$rustsec_db" "$RUSTSEC_DB_COMMIT" "$RUSTSEC_DB_TREE" \
  || fail "fleet-governed RustSec database custody is invalid"

cargo_cache_home="${CARGO_HOME:-${HOME}/.cargo}"
[[ "$cargo_cache_home" == /* && -d "$cargo_cache_home" \
  && ! -L "$cargo_cache_home" \
  && "$(realpath -e -- "$cargo_cache_home")" == "$cargo_cache_home" ]] \
  || fail "governed Cargo home must be a physical absolute directory"
host_cargo_cache_parent="$cargo_cache_home/registry/cache"
host_cargo_cache="$host_cargo_cache_parent/index.crates.io-1949cf8c6b5b557f"
host_cargo_index="$cargo_cache_home/registry/index/index.crates.io-1949cf8c6b5b557f"
host_cargo_index_manifest_sha256="$(jain_directory_manifest_sha256 "$host_cargo_index")"
[[ "$host_cargo_index_manifest_sha256" =~ ^[0-9a-f]{64}$ ]] \
  || fail "governed Cargo index manifest is invalid"
jain_verify_locked_cargo_registry_closure \
  "$ROOT_DIR/Cargo.lock" "$host_cargo_cache" "$cargo_cache_home" \
  || fail "governed Cargo.lock archive closure is incomplete"

export CARGO_NET_OFFLINE=true
export SYFT_CHECK_FOR_APP_UPDATE=false
artifact_root="${ARTIFACT_DIR}/security"
mkdir -p "$artifact_root"
rustsec_archive="${artifact_root}/rustsec-db.tar"
rustsec_snapshot="${artifact_root}/rustsec-db"
rm -rf -- "$rustsec_snapshot"
mkdir -p "$rustsec_snapshot"
git -C "$rustsec_db" archive --format=tar \
  --output "$rustsec_archive" "$RUSTSEC_DB_COMMIT"
rustsec_archive_sha256="$(jain_sha256 "$rustsec_archive")"
tar -xf "$rustsec_archive" -C "$rustsec_snapshot"
[[ -z "$(find "$rustsec_snapshot" -type l -print -quit)" ]] \
  || fail "pinned RustSec snapshot contains a symlink"
rustsec_tree="$(git -C "$rustsec_db" rev-parse "${RUSTSEC_DB_COMMIT}^{tree}")"
[[ "$rustsec_tree" == "$RUSTSEC_DB_TREE" ]] \
  || fail "pinned RustSec tree identity mismatch"

log "security: pinned local gitleaks"
"$gitleaks_bin" detect --source . --config gitleaks.toml --no-banner --redact \
  --report-format json --report-path "$artifact_root/gitleaks.json"
jq -e 'type == "array" and length == 0' "$artifact_root/gitleaks.json" >/dev/null

log "security: pinned local RustSec database"
"$cargo_audit_bin" audit --no-fetch --db "$rustsec_snapshot" --deny warnings --json \
  >"$artifact_root/cargo-audit.json"
jq -e '.vulnerabilities.found == false and (.vulnerabilities.list | length) == 0
  and ((.warnings // {}) | length == 0)' "$artifact_root/cargo-audit.json" >/dev/null

log "security: pinned local cargo-deny"
cargo_metadata="$artifact_root/cargo-metadata.json"
CARGO_HOME="$cargo_cache_home" cargo metadata --locked --offline --format-version 1 \
  >"$cargo_metadata"
cargo_deny_home="$(mktemp -d "$artifact_root/cargo-deny-home.XXXXXX")"
jain_seed_cargo_registry_index \
  "$host_cargo_index" "$cargo_deny_home" "$host_cargo_index_manifest_sha256" \
  || fail "security: exact isolated crates.io index seed failed"
jain_seed_locked_cargo_registry_closure \
  "$ROOT_DIR/Cargo.lock" "$host_cargo_cache_parent" "$host_cargo_cache" \
  "$cargo_deny_home" \
  || fail "security: exact Cargo.lock registry closure seed failed"
jain_seed_cargo_deny_advisory_db \
  "$rustsec_db" "$cargo_deny_home" "$RUSTSEC_DB_COMMIT" "$RUSTSEC_DB_TREE" \
  || fail "security: exact isolated cargo-deny advisory DB seed failed"
cargo_deny_log="$artifact_root/cargo-deny.log"
if ! CARGO_HOME="$cargo_deny_home" "$cargo_deny_bin" check \
  --metadata-path "$cargo_metadata" --disable-fetch --deny warnings \
  >"$cargo_deny_log" 2>&1; then
  cat "$cargo_deny_log" >&2
  fail "security: cargo-deny failed; log preserved at $cargo_deny_log"
fi
jain_verify_cargo_deny_clean_log "$cargo_deny_log" || {
  cat "$cargo_deny_log" >&2
  fail "security: cargo-deny output was not the exact passing summary; log preserved at $cargo_deny_log"
}
jain_verify_locked_cargo_registry_closure \
  "$ROOT_DIR/Cargo.lock" "$host_cargo_cache" "$cargo_deny_home" \
  || fail "security: cargo-deny mutated its exact Cargo.lock archive closure"
jain_verify_isolated_cargo_deny_db \
  "$cargo_deny_home/advisory-dbs/advisory-db-3157b0e258782691" \
  "$RUSTSEC_DB_COMMIT" "$RUSTSEC_DB_TREE" \
  || fail "security: cargo-deny mutated its isolated advisory DB"
[[ -z "$(find "$cargo_deny_home" -type l -print -quit)" ]] \
  || fail "security: isolated cargo-deny home contains a symlink"
jain_verify_governed_advisory_db \
  "$rustsec_db" "$RUSTSEC_DB_COMMIT" "$RUSTSEC_DB_TREE" \
  || fail "fleet-governed RustSec identity changed during scanning"
rm -rf -- "$cargo_deny_home"
cargo_deny_home=""

log "security: offline npm advisory cache"
(cd "$WEB_DIR" && npm audit --offline --audit-level=high --json) \
  >"$artifact_root/npm-audit.json"
jq -e '.metadata.vulnerabilities.high == 0
  and .metadata.vulnerabilities.critical == 0' "$artifact_root/npm-audit.json" >/dev/null

log "security: pinned offline blocking zizmor"
"$zizmor_bin" --offline --format json --no-progress .github/workflows \
  >"$artifact_root/zizmor.json"
jq -e 'type == "array" and length == 0' "$artifact_root/zizmor.json" >/dev/null

log "security: pinned update-disabled local Syft SBOM"
"$syft_bin" scan dir:. --config "$SYFT_CONFIG" -q \
  --exclude './.git/**' --exclude './target/**' \
  --source-name redline-web --source-version "$(git rev-parse HEAD)" \
  --output "spdx-json=$artifact_root/redline-web.spdx.json"
jq -e '.spdxVersion | startswith("SPDX-")' \
  "$artifact_root/redline-web.spdx.json" >/dev/null

jq -n \
  --arg commit "$(git rev-parse HEAD)" \
  --arg tree "$(git rev-parse 'HEAD^{tree}')" \
  --arg rustsec_commit "$RUSTSEC_DB_COMMIT" \
  --arg rustsec_tree "$rustsec_tree" \
  --arg rustsec_archive_sha256 "$rustsec_archive_sha256" \
  --arg cargo_index_manifest_sha256 "$host_cargo_index_manifest_sha256" \
  --arg cargo_audit_sha256 "$CARGO_AUDIT_SHA256" \
  --arg cargo_deny_sha256 "$CARGO_DENY_SHA256" \
  --arg gitleaks_sha256 "$GITLEAKS_SHA256" \
  --arg zizmor_sha256 "$ZIZMOR_SHA256" \
  --arg syft_sha256 "$SYFT_SHA256" \
  '{schema_version:"redline.web.security/v1",status:"pass",network:"offline",
    commit:$commit,tree:$tree,rustsec_db_commit:$rustsec_commit,
    rustsec_db_tree:$rustsec_tree,rustsec_db_archive_sha256:$rustsec_archive_sha256,
    cargo_index_manifest_sha256:$cargo_index_manifest_sha256,
    tool_sha256:{cargo_audit:$cargo_audit_sha256,cargo_deny:$cargo_deny_sha256,
      gitleaks:$gitleaks_sha256,zizmor:$zizmor_sha256,syft:$syft_sha256},
    checks:["gitleaks","cargo-audit-pinned-no-fetch","cargo-deny-no-fetch",
      "npm-audit-offline","zizmor-offline-strict","syft-pinned-update-disabled"]}' \
  >"$artifact_root/source-evidence.json"
log "security: complete"
