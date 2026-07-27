#!/usr/bin/env bash
# Fully local, pinned, fail-closed supply-chain evidence.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"

CARGO_AUDIT_BIN="$(command -v cargo-audit 2>/dev/null || true)"
[[ -n "$CARGO_AUDIT_BIN" ]] || CARGO_AUDIT_BIN="/home/ubuntu/.cargo/bin/cargo-audit"
readonly CARGO_AUDIT_BIN
readonly CARGO_AUDIT_SHA256="1a17ff4c0449d1924aacda8dd20c06dccc3cceeed4dd17a71523f672bf97b70b"
CARGO_DENY_BIN="$(command -v cargo-deny 2>/dev/null || true)"
[[ -n "$CARGO_DENY_BIN" ]] || CARGO_DENY_BIN="/home/ubuntu/.cargo/bin/cargo-deny"
readonly CARGO_DENY_BIN
readonly CARGO_DENY_SHA256="ef27c757f50d77c5c2d9114fbc6ad45d2b8903506cead473a70b8ee659ea7a18"
GITLEAKS_BIN="$(command -v gitleaks 2>/dev/null || true)"
[[ -n "$GITLEAKS_BIN" ]] || GITLEAKS_BIN="/home/ubuntu/.cargo/bin/gitleaks"
readonly GITLEAKS_BIN
readonly GITLEAKS_SHA256="50b742abd7daad8bbddb6301f3017efb680632d9a5b3b4d8f137b3aac250e359"
ZIZMOR_BIN="$(command -v zizmor 2>/dev/null || true)"
[[ -n "$ZIZMOR_BIN" ]] || ZIZMOR_BIN="/home/ubuntu/.cargo/bin/zizmor"
readonly ZIZMOR_BIN
readonly ZIZMOR_SHA256="6eabb307e2c0c35aa3b397d35518db66106c6909f86e7e0d7d898bc6587aa7fc"
SYFT_BIN="$(command -v syft 2>/dev/null || true)"
[[ -n "$SYFT_BIN" ]] || SYFT_BIN="/home/ubuntu/.local/bin/syft"
readonly SYFT_BIN
readonly SYFT_SHA256="eb9714fb8e4b8f2a647e7bb312f1e0b9f83a7aa30418658bf46583cfa83d27d2"
readonly SYFT_CONFIG="${ROOT_DIR}/ops/ci/syft.yaml"
RUSTSEC_DB="$(jain_first_present_path \
  "${JAIN_RUSTSEC_ADVISORY_SOURCE:-}" "${JAIN_PINNED_ADVISORY_DB:-}" \
  "${JAIN_ADVISORY_DB:-}" /home/ubuntu/.cargo/advisory-db || true)"
readonly RUSTSEC_DB
readonly RUSTSEC_DB_COMMIT="9f3e138091487e69144f536d36976e427a7a3307"
readonly RUSTSEC_DB_TREE="c33f1047906505cabcec7e21f2d99db5c6de8852"
CARGO_DENY_DB="$(jain_first_present_path \
  "${JAIN_CARGO_DENY_ADVISORY_DB:-}" \
  /home/ubuntu/.cargo/advisory-dbs/advisory-db-3157b0e258782691 || true)"
readonly CARGO_DENY_DB
HOST_CARGO_REGISTRY="$(jain_first_present_path \
  /opt/jain-ci/cargo-registry /var/lib/jain-host-ci/cargo-registry \
  /home/ubuntu/.cargo/registry || true)"
readonly HOST_CARGO_REGISTRY
readonly HOST_CARGO_CACHE_PARENT="${HOST_CARGO_REGISTRY}/cache"
readonly HOST_CARGO_CACHE="${HOST_CARGO_CACHE_PARENT}/index.crates.io-1949cf8c6b5b557f"
# The crates.io index is seeded from the Cargo.lock closure only. The developer
# cache is NOT an authoritative custody object -- it is shared and appendable, so
# the whole-directory digest pin this replaces fired on ordinary resolution
# elsewhere on the host. cargo-deny still needs index entries to answer whether a
# locked version is yanked, so the entries are scoped to committed lock bytes
# rather than dropped.
readonly HOST_CARGO_INDEX="${HOST_CARGO_REGISTRY}/index/index.crates.io-1949cf8c6b5b557f"
# SHA-256 over config.json plus the sorted relative paths and bytes of ONLY the
# Cargo.lock-selected index entries. Unrelated shared-cache growth cannot move
# it; an upstream change to a selected crate requires a reviewed bump here.
readonly LOCK_CLOSURE_INDEX_MANIFEST_SHA256="45169acea56070f13e18ce19f2f4d3ec86abaec0cf47365b9f27b31505b2abb3"
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
jain_verify_exact_executable cargo-audit "$CARGO_AUDIT_BIN" "$CARGO_AUDIT_SHA256" || exit 1
jain_verify_exact_executable cargo-deny "$CARGO_DENY_BIN" "$CARGO_DENY_SHA256" || exit 1
jain_verify_exact_executable gitleaks "$GITLEAKS_BIN" "$GITLEAKS_SHA256" || exit 1
jain_verify_exact_executable zizmor "$ZIZMOR_BIN" "$ZIZMOR_SHA256" || exit 1
jain_verify_exact_executable syft "$SYFT_BIN" "$SYFT_SHA256" || exit 1
[[ -f "$SYFT_CONFIG" && ! -L "$SYFT_CONFIG" ]] \
  || fail "pinned local Syft configuration is missing or symlinked"

[[ -d "$RUSTSEC_DB" && ! -L "$RUSTSEC_DB" \
  && "$(realpath -e -- "$RUSTSEC_DB")" == "$RUSTSEC_DB" \
  && -d "$RUSTSEC_DB/.git" && ! -L "$RUSTSEC_DB/.git" ]] \
  || fail "pinned local RustSec database custody is invalid"
git -C "$RUSTSEC_DB" cat-file -e "${RUSTSEC_DB_COMMIT}^{commit}" \
  || fail "pinned local RustSec commit object is unavailable"

export CARGO_NET_OFFLINE=true
export SYFT_CHECK_FOR_APP_UPDATE=false
artifact_root="${ARTIFACT_DIR}/security"
mkdir -p "$artifact_root"
rustsec_archive="${artifact_root}/rustsec-db.tar"
rustsec_snapshot="${artifact_root}/rustsec-db"
rm -rf -- "$rustsec_snapshot"
mkdir -p "$rustsec_snapshot"
git -C "$RUSTSEC_DB" archive --format=tar \
  --output "$rustsec_archive" "$RUSTSEC_DB_COMMIT"
rustsec_archive_sha256="$(jain_sha256 "$rustsec_archive")"
tar -xf "$rustsec_archive" -C "$rustsec_snapshot"
[[ -z "$(find "$rustsec_snapshot" -type l -print -quit)" ]] \
  || fail "pinned RustSec snapshot contains a symlink"
rustsec_tree="$(git -C "$RUSTSEC_DB" rev-parse "${RUSTSEC_DB_COMMIT}^{tree}")"
[[ "$rustsec_tree" == "$RUSTSEC_DB_TREE" ]] \
  || fail "pinned RustSec tree identity mismatch"

log "security: pinned local gitleaks"
"$GITLEAKS_BIN" detect --source . --config gitleaks.toml --no-banner --redact \
  --report-format json --report-path "$artifact_root/gitleaks.json"
jq -e 'type == "array" and length == 0' "$artifact_root/gitleaks.json" >/dev/null

log "security: pinned local RustSec database"
"$CARGO_AUDIT_BIN" audit --no-fetch --db "$rustsec_snapshot" --deny warnings --json \
  >"$artifact_root/cargo-audit.json"
jq -e '.vulnerabilities.found == false and (.vulnerabilities.list | length) == 0
  and ((.warnings // {}) | length == 0)' "$artifact_root/cargo-audit.json" >/dev/null

log "security: pinned local cargo-deny"
cargo_cache_home="${CARGO_HOME:-${HOME}/.cargo}"
jain_seed_locked_cargo_archives \
  "$ROOT_DIR/Cargo.lock" "$HOST_CARGO_CACHE_PARENT" "$HOST_CARGO_CACHE" \
  "$cargo_cache_home" valuable 0.1.1 anstyle-wincon 3.0.11 \
  || fail "security: exact locked cargo-deny cache seed failed"
cargo_metadata="$artifact_root/cargo-metadata.json"
CARGO_HOME="$cargo_cache_home" cargo metadata --locked --offline --format-version 1 \
  >"$cargo_metadata"
cargo_deny_home="$(mktemp -d "$artifact_root/cargo-deny-home.XXXXXX")"
jain_seed_locked_cargo_registry_index \
  "$ROOT_DIR/Cargo.lock" "$HOST_CARGO_INDEX" "$cargo_deny_home" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" \
  || fail "security: lock-closure crates.io index seed failed"
jain_seed_locked_cargo_registry_closure \
  "$ROOT_DIR/Cargo.lock" "$HOST_CARGO_CACHE_PARENT" "$HOST_CARGO_CACHE" \
  "$cargo_deny_home" \
  || fail "security: exact Cargo.lock registry closure seed failed"
jain_seed_cargo_deny_advisory_db \
  "$CARGO_DENY_DB" "$cargo_deny_home" "$RUSTSEC_DB_COMMIT" "$RUSTSEC_DB_TREE" \
  || fail "security: exact isolated cargo-deny advisory DB seed failed"
cargo_deny_log="$artifact_root/cargo-deny.log"
if ! CARGO_HOME="$cargo_deny_home" "$CARGO_DENY_BIN" check \
  --metadata-path "$cargo_metadata" --disable-fetch --deny warnings \
  >"$cargo_deny_log" 2>&1; then
  cat "$cargo_deny_log" >&2
  fail "security: cargo-deny failed; log preserved at $cargo_deny_log"
fi
jain_verify_locked_cargo_registry_index \
  "$ROOT_DIR/Cargo.lock" \
  "$cargo_deny_home/registry/index/$(basename -- "$HOST_CARGO_INDEX")" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" \
  || fail "security: isolated index changed while cargo-deny ran"
jain_verify_cargo_deny_clean_log "$cargo_deny_log" || {
  cat "$cargo_deny_log" >&2
  fail "security: cargo-deny output was not the exact passing summary; log preserved at $cargo_deny_log"
}
jain_verify_locked_cargo_registry_closure \
  "$ROOT_DIR/Cargo.lock" "$HOST_CARGO_CACHE" "$cargo_deny_home" \
  || fail "security: cargo-deny mutated its exact Cargo.lock archive closure"
jain_verify_isolated_cargo_deny_db \
  "$cargo_deny_home/advisory-dbs/advisory-db-3157b0e258782691" \
  "$RUSTSEC_DB_COMMIT" "$RUSTSEC_DB_TREE" \
  || fail "security: cargo-deny mutated its isolated advisory DB"
[[ -z "$(find "$cargo_deny_home" -type l -print -quit)" ]] \
  || fail "security: isolated cargo-deny home contains a symlink"
rm -rf -- "$cargo_deny_home"
cargo_deny_home=""

log "security: offline npm advisory cache"
(cd "$WEB_DIR" && npm audit --offline --audit-level=high --json) \
  >"$artifact_root/npm-audit.json"
jq -e '.metadata.vulnerabilities.high == 0
  and .metadata.vulnerabilities.critical == 0' "$artifact_root/npm-audit.json" >/dev/null

log "security: pinned offline blocking zizmor"
"$ZIZMOR_BIN" --offline --format json --no-progress .github/workflows \
  >"$artifact_root/zizmor.json"
jq -e 'type == "array" and length == 0' "$artifact_root/zizmor.json" >/dev/null

log "security: pinned update-disabled local Syft SBOM"
"$SYFT_BIN" scan dir:. --config "$SYFT_CONFIG" -q \
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
  --arg cargo_audit_sha256 "$CARGO_AUDIT_SHA256" \
  --arg cargo_deny_sha256 "$CARGO_DENY_SHA256" \
  --arg gitleaks_sha256 "$GITLEAKS_SHA256" \
  --arg zizmor_sha256 "$ZIZMOR_SHA256" \
  --arg syft_sha256 "$SYFT_SHA256" \
  '{schema_version:"redline.web.security/v1",status:"pass",network:"offline",
    commit:$commit,tree:$tree,rustsec_db_commit:$rustsec_commit,
    rustsec_db_tree:$rustsec_tree,rustsec_db_archive_sha256:$rustsec_archive_sha256,
    tool_sha256:{cargo_audit:$cargo_audit_sha256,cargo_deny:$cargo_deny_sha256,
      gitleaks:$gitleaks_sha256,zizmor:$zizmor_sha256,syft:$syft_sha256},
    checks:["gitleaks","cargo-audit-pinned-no-fetch","cargo-deny-no-fetch",
      "npm-audit-offline","zizmor-offline-strict","syft-pinned-update-disabled"]}' \
  >"$artifact_root/source-evidence.json"
log "security: complete"
