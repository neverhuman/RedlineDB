#!/usr/bin/env bash
# Fully local, pinned, fail-closed supply-chain evidence.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"

CARGO_AUDIT_BIN="$(command -v cargo-audit 2>/dev/null || true)"
[[ -n "$CARGO_AUDIT_BIN" && -f "$CARGO_AUDIT_BIN" \
  && ! -L "$CARGO_AUDIT_BIN" ]] \
  || CARGO_AUDIT_BIN="/home/ubuntu/.cargo/bin/cargo-audit"
readonly CARGO_AUDIT_BIN
CARGO_AUDIT_IDENTITY="${JAIN_REAL_CARGO_AUDIT:-$CARGO_AUDIT_BIN}"
readonly CARGO_AUDIT_IDENTITY
readonly CARGO_AUDIT_SHA256="1a17ff4c0449d1924aacda8dd20c06dccc3cceeed4dd17a71523f672bf97b70b"
CARGO_DENY_BIN="$(command -v cargo-deny 2>/dev/null || true)"
[[ -n "$CARGO_DENY_BIN" && -f "$CARGO_DENY_BIN" \
  && ! -L "$CARGO_DENY_BIN" ]] \
  || CARGO_DENY_BIN="/home/ubuntu/.cargo/bin/cargo-deny"
readonly CARGO_DENY_BIN
CARGO_DENY_IDENTITY="${JAIN_REAL_CARGO_DENY:-$CARGO_DENY_BIN}"
readonly CARGO_DENY_IDENTITY
readonly CARGO_DENY_SHA256="ef27c757f50d77c5c2d9114fbc6ad45d2b8903506cead473a70b8ee659ea7a18"
GITLEAKS_BIN="$(command -v gitleaks 2>/dev/null || true)"
[[ -n "$GITLEAKS_BIN" && -f "$GITLEAKS_BIN" && ! -L "$GITLEAKS_BIN" ]] \
  || GITLEAKS_BIN="/home/ubuntu/.cargo/bin/gitleaks"
readonly GITLEAKS_BIN
readonly GITLEAKS_SHA256="50b742abd7daad8bbddb6301f3017efb680632d9a5b3b4d8f137b3aac250e359"
ZIZMOR_BIN="$(command -v zizmor 2>/dev/null || true)"
[[ -n "$ZIZMOR_BIN" && -f "$ZIZMOR_BIN" && ! -L "$ZIZMOR_BIN" ]] \
  || ZIZMOR_BIN="/home/ubuntu/.cargo/bin/zizmor"
readonly ZIZMOR_BIN
readonly ZIZMOR_SHA256="6eabb307e2c0c35aa3b397d35518db66106c6909f86e7e0d7d898bc6587aa7fc"
SYFT_BIN="$(command -v syft 2>/dev/null || true)"
[[ -n "$SYFT_BIN" ]] || SYFT_BIN="/home/ubuntu/.local/bin/syft"
readonly SYFT_BIN
readonly SYFT_SHA256="eb9714fb8e4b8f2a647e7bb312f1e0b9f83a7aa30418658bf46583cfa83d27d2"
readonly SYFT_CONFIG="${ROOT_DIR}/ops/ci/syft.yaml"
GRYPE_BIN="$(command -v grype 2>/dev/null || true)"
[[ -n "$GRYPE_BIN" ]] || GRYPE_BIN="/home/ubuntu/.local/bin/grype"
readonly GRYPE_BIN
readonly GRYPE_SHA256="ba5cdfb57056c8a68c313c8ed78e26567b310ade7b251cba03af582c3ed88672"
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
readonly GOVERNED_CARGO_REGISTRY="/var/lib/jain-host-ci/cargo-registry"
readonly DEVELOPER_CARGO_REGISTRY="/home/ubuntu/.cargo/registry"
readonly GOVERNED_INDEX_MANIFEST_SHA256="45169acea56070f13e18ce19f2f4d3ec86abaec0cf47365b9f27b31505b2abb3"
readonly DEVELOPER_INDEX_MANIFEST_SHA256="aac1119a086ca336165f570d5794853f0dfedb444ed4300cf13a77bac170ffad"
if [[ "${JAIN_RELEASE_CI:-0}" == 1 ]]; then
  [[ -n "${CARGO_HOME:-}" && -d "$CARGO_HOME/registry" \
    && ! -L "$CARGO_HOME" && ! -L "$CARGO_HOME/registry" \
    && "$(realpath -e -- "$CARGO_HOME/registry")" == "$CARGO_HOME/registry" \
    && -f "$CARGO_HOME/registry/stage-receipt.json" \
    && ! -L "$CARGO_HOME/registry/stage-receipt.json" \
    && -f "$CARGO_HOME/registry/lock-source-closure.json" \
    && ! -L "$CARGO_HOME/registry/lock-source-closure.json" ]] \
    || fail "release security requires the fresh root-staged CARGO_HOME registry"
  HOST_CARGO_REGISTRY="$CARGO_HOME/registry"
  CARGO_REGISTRY_TRUST_DOMAIN="release-root-staged"
  LOCK_CLOSURE_INDEX_MANIFEST_SHA256="$GOVERNED_INDEX_MANIFEST_SHA256"
else
  case "${REDLINE_LOCAL_CARGO_REGISTRY_TRUST_DOMAIN:-governed}" in
    governed)
      if [[ -d "$GOVERNED_CARGO_REGISTRY" ]]; then
        HOST_CARGO_REGISTRY="$GOVERNED_CARGO_REGISTRY"
        CARGO_REGISTRY_TRUST_DOMAIN="local-governed"
        LOCK_CLOSURE_INDEX_MANIFEST_SHA256="$GOVERNED_INDEX_MANIFEST_SHA256"
      else
        HOST_CARGO_REGISTRY="$DEVELOPER_CARGO_REGISTRY"
        CARGO_REGISTRY_TRUST_DOMAIN="local-developer-fallback"
        LOCK_CLOSURE_INDEX_MANIFEST_SHA256="$DEVELOPER_INDEX_MANIFEST_SHA256"
      fi
      ;;
    developer)
      HOST_CARGO_REGISTRY="$DEVELOPER_CARGO_REGISTRY"
      CARGO_REGISTRY_TRUST_DOMAIN="local-developer-explicit"
      LOCK_CLOSURE_INDEX_MANIFEST_SHA256="$DEVELOPER_INDEX_MANIFEST_SHA256"
      ;;
    *)
      fail "local Cargo registry trust domain must be governed or developer"
      ;;
  esac
fi
readonly HOST_CARGO_REGISTRY
readonly CARGO_REGISTRY_TRUST_DOMAIN
readonly LOCK_CLOSURE_INDEX_MANIFEST_SHA256
readonly HOST_CARGO_CACHE_PARENT="${HOST_CARGO_REGISTRY}/cache"
readonly HOST_CARGO_CACHE="${HOST_CARGO_CACHE_PARENT}/index.crates.io-1949cf8c6b5b557f"
readonly HOST_CARGO_INDEX="${HOST_CARGO_REGISTRY}/index/index.crates.io-1949cf8c6b5b557f"
readonly LOCAL_GRYPE_DB_INVENTORY_SHA256="3f673a9c1e40b6181fb504d0d061973490777160acc42ccc126453fda27f2db4"
readonly LOCAL_GRYPE_DB_ROOT="/var/lib/jain-host-ci/grype-db/${LOCAL_GRYPE_DB_INVENTORY_SHA256}"
if [[ "${JAIN_RELEASE_CI:-0}" == 1 ]]; then
  [[ "${JAIN_GRYPE_DB_ROOT:-}" == /opt/jain-ci/grype-db \
    && "${JAIN_GRYPE_DB_INVENTORY_SHA256:-}" =~ ^[0-9a-f]{64}$ \
    && "${GRYPE_DB_CACHE_DIR:-}" == "$JAIN_GRYPE_DB_ROOT" ]] \
    || fail "release Grype database authority is incomplete or unbound"
  GRYPE_DB_ROOT="$JAIN_GRYPE_DB_ROOT"
  GRYPE_DB_INVENTORY_SHA256="$JAIN_GRYPE_DB_INVENTORY_SHA256"
else
  [[ -z "${JAIN_GRYPE_DB_ROOT:-}" \
    || "${JAIN_GRYPE_DB_ROOT:-}" == "$LOCAL_GRYPE_DB_ROOT" ]] \
    || fail "local Grype database override is not the governed authority"
  [[ -z "${JAIN_GRYPE_DB_INVENTORY_SHA256:-}" \
    || "${JAIN_GRYPE_DB_INVENTORY_SHA256:-}" == "$LOCAL_GRYPE_DB_INVENTORY_SHA256" ]] \
    || fail "local Grype database inventory override is not governed"
  [[ -z "${GRYPE_DB_CACHE_DIR:-}" \
    || "${GRYPE_DB_CACHE_DIR:-}" == "$LOCAL_GRYPE_DB_ROOT" ]] \
    || fail "local Grype cache is unbound from the governed authority"
  GRYPE_DB_ROOT="$LOCAL_GRYPE_DB_ROOT"
  GRYPE_DB_INVENTORY_SHA256="$LOCAL_GRYPE_DB_INVENTORY_SHA256"
fi
readonly GRYPE_DB_ROOT
readonly GRYPE_DB_INVENTORY_SHA256
readonly EXPECTED_NPM_PACKAGE_COUNT=376
readonly EXPECTED_SECURITY_COMMANDS="gitleaks detect; cargo audit; cargo deny; zizmor; syft; grype"

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

for tool in cmp diff find git jq sha256sum realpath stat tar; do
  has "$tool" || fail "required local security tool is missing: $tool"
done
[[ -z "$(git status --porcelain)" ]] || fail "security evidence requires a clean checkout"
jain_verify_exact_executable cargo-audit "$CARGO_AUDIT_IDENTITY" "$CARGO_AUDIT_SHA256" || exit 1
jain_verify_exact_executable cargo-deny "$CARGO_DENY_IDENTITY" "$CARGO_DENY_SHA256" || exit 1
jain_verify_exact_executable gitleaks "$GITLEAKS_BIN" "$GITLEAKS_SHA256" || exit 1
jain_verify_exact_executable zizmor "$ZIZMOR_BIN" "$ZIZMOR_SHA256" || exit 1
jain_verify_exact_executable syft "$SYFT_BIN" "$SYFT_SHA256" || exit 1
jain_verify_exact_executable grype "$GRYPE_BIN" "$GRYPE_SHA256" || exit 1
[[ -f "$SYFT_CONFIG" && ! -L "$SYFT_CONFIG" ]] \
  || fail "pinned local Syft configuration is missing or symlinked"
if [[ "${JAIN_RELEASE_CI:-0}" == 1 ]]; then
  [[ -n "${JAIN_REAL_CARGO_AUDIT:-}" && -n "${JAIN_REAL_CARGO_DENY:-}" \
    && "$CARGO_AUDIT_BIN" != "$CARGO_AUDIT_IDENTITY" \
    && "$CARGO_DENY_BIN" != "$CARGO_DENY_IDENTITY" \
    && -f "$CARGO_AUDIT_BIN" && ! -L "$CARGO_AUDIT_BIN" && -x "$CARGO_AUDIT_BIN" \
    && -f "$CARGO_DENY_BIN" && ! -L "$CARGO_DENY_BIN" && -x "$CARGO_DENY_BIN" ]] \
    || fail "release RustSec wrapper/real-binary authority is incomplete"
  [[ "${JAIN_RUSTSEC_ADVISORY_SOURCE:-}" == "${JAIN_PINNED_ADVISORY_DB:-}" \
    && "${JAIN_ADVISORY_DB:-}" == "${JAIN_PINNED_ADVISORY_DB:-}" \
    && "$RUSTSEC_DB" == "${JAIN_PINNED_ADVISORY_DB:-}" \
    && "$CARGO_DENY_DB" == "${JAIN_CARGO_DENY_ADVISORY_DB:-}" \
    && "${JAIN_PINNED_ADVISORY_COMMIT:-}" == "$RUSTSEC_DB_COMMIT" ]] \
    || fail "release RustSec exports differ from the pinned root authority"
fi

[[ -d "$RUSTSEC_DB" && ! -L "$RUSTSEC_DB" \
  && "$(realpath -e -- "$RUSTSEC_DB")" == "$RUSTSEC_DB" \
  && -d "$RUSTSEC_DB/.git" && ! -L "$RUSTSEC_DB/.git" ]] \
  || fail "pinned local RustSec database custody is invalid"
git -C "$RUSTSEC_DB" cat-file -e "${RUSTSEC_DB_COMMIT}^{commit}" \
  || fail "pinned local RustSec commit object is unavailable"
[[ -d "$CARGO_DENY_DB" && ! -L "$CARGO_DENY_DB" \
  && "$(realpath -e -- "$CARGO_DENY_DB")" == "$CARGO_DENY_DB" \
  && -d "$CARGO_DENY_DB/.git" && ! -L "$CARGO_DENY_DB/.git" ]] \
  || fail "pinned cargo-deny RustSec custody is invalid"

export CARGO_NET_OFFLINE=true
export SYFT_CHECK_FOR_APP_UPDATE=false
export GRYPE_CHECK_FOR_APP_UPDATE=false
export GRYPE_DB_AUTO_UPDATE=false
export GRYPE_DB_CACHE_DIR="$GRYPE_DB_ROOT"
artifact_root="${ARTIFACT_DIR}/security"
mkdir -p "$artifact_root"
rustsec_tree="$(git -C "$RUSTSEC_DB" rev-parse "${RUSTSEC_DB_COMMIT}^{tree}")"
[[ "$rustsec_tree" == "$RUSTSEC_DB_TREE" ]] \
  || fail "pinned RustSec tree identity mismatch"
rustsec_archive_sha256=""
audit_db="$RUSTSEC_DB"
if [[ "${JAIN_RELEASE_CI:-0}" != 1 ]]; then
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
  audit_db="$rustsec_snapshot"
fi

jain_verify_grype_db_authority \
  "$GRYPE_DB_ROOT" "$GRYPE_DB_INVENTORY_SHA256" \
  || fail "security: governed Grype database authority failed validation"
grype_db_status="$artifact_root/grype-db-status.json"
"$GRYPE_BIN" db status -o json >"$grype_db_status"
jain_verify_grype_db_status "$grype_db_status" "$GRYPE_DB_ROOT" \
  || fail "security: Grype database status is invalid or unbound"

log "security: pinned local gitleaks"
"$GITLEAKS_BIN" detect --source . --config gitleaks.toml --no-banner --redact \
  --report-format json --report-path "$artifact_root/gitleaks.json"
jq -e 'type == "array" and length == 0' "$artifact_root/gitleaks.json" >/dev/null

log "security: pinned local RustSec database"
"$CARGO_AUDIT_BIN" audit --no-fetch --db "$audit_db" --deny warnings --json \
  >"$artifact_root/cargo-audit.json"
jq -e '.vulnerabilities.found == false and (.vulnerabilities.list | length) == 0
  and ((.warnings // {}) | length == 0)' "$artifact_root/cargo-audit.json" >/dev/null

log "security: pinned offline cargo-deny trust-domain=$CARGO_REGISTRY_TRUST_DOMAIN"
cargo_metadata="$artifact_root/cargo-metadata.json"
cargo_stage_receipt_sha256=""
cargo_lock_closure_sha256=""
if [[ "${JAIN_RELEASE_CI:-0}" == 1 ]]; then
  cargo_deny_home="$CARGO_HOME"
  jain_verify_staged_cargo_registry \
    "$ROOT_DIR/Cargo.lock" "$HOST_CARGO_REGISTRY" \
    "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" \
    || fail "security: root-staged Cargo registry failed pre-deny validation"
  cargo_stage_receipt_sha256="$(jain_sha256 \
    "$HOST_CARGO_REGISTRY/stage-receipt.json")"
  cargo_lock_closure_sha256="$(jain_sha256 \
    "$HOST_CARGO_REGISTRY/lock-source-closure.json")"
else
  cargo_cache_home="${CARGO_HOME:-${HOME}/.cargo}"
  jain_seed_locked_cargo_archives \
    "$ROOT_DIR/Cargo.lock" "$HOST_CARGO_CACHE_PARENT" "$HOST_CARGO_CACHE" \
    "$cargo_cache_home" valuable 0.1.1 anstyle-wincon 3.0.11 \
    || fail "security: exact locked cargo-deny cache seed failed"
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
fi
cargo_inventory_before="$(
  jain_staged_cargo_registry_inventory_sha256 "$cargo_deny_home/registry"
)" || fail "security: cannot snapshot Cargo registry before cargo-deny"
CARGO_HOME="$cargo_deny_home" cargo metadata --locked --offline --format-version 1 \
  >"$cargo_metadata"
cargo_deny_log="$artifact_root/cargo-deny.log"
if ! CARGO_HOME="$cargo_deny_home" "$CARGO_DENY_BIN" check \
  --metadata-path "$cargo_metadata" --disable-fetch --deny warnings \
  >"$cargo_deny_log" 2>&1; then
  cat "$cargo_deny_log" >&2
  fail "security: cargo-deny failed; log preserved at $cargo_deny_log"
fi
if [[ "${JAIN_RELEASE_CI:-0}" == 1 ]]; then
  post_index_manifest="$(jain_cargo_index_closure_manifest_sha256 \
    "$ROOT_DIR/Cargo.lock" \
    "$cargo_deny_home/registry/index/$(basename -- "$HOST_CARGO_INDEX")")" \
    || fail "security: cannot recompute staged selected-index manifest"
  [[ "$post_index_manifest" == "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" ]] \
    || fail "security: staged selected-index manifest changed during cargo-deny"
else
  jain_verify_locked_cargo_registry_index \
    "$ROOT_DIR/Cargo.lock" \
    "$cargo_deny_home/registry/index/$(basename -- "$HOST_CARGO_INDEX")" \
    "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" \
    || fail "security: isolated index changed while cargo-deny ran"
fi
jain_verify_cargo_deny_clean_log "$cargo_deny_log" || {
  cat "$cargo_deny_log" >&2
  fail "security: cargo-deny output was not the exact passing summary; log preserved at $cargo_deny_log"
}
cargo_inventory_after="$(
  jain_staged_cargo_registry_inventory_sha256 "$cargo_deny_home/registry"
)" || fail "security: cannot snapshot Cargo registry after cargo-deny"
[[ "$cargo_inventory_after" == "$cargo_inventory_before" ]] \
  || fail "security: cargo-deny mutated the staged Cargo registry inventory"
if [[ "${JAIN_RELEASE_CI:-0}" == 1 ]]; then
  jain_verify_staged_cargo_registry \
    "$ROOT_DIR/Cargo.lock" "$HOST_CARGO_REGISTRY" \
    "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" \
    || fail "security: root-staged Cargo registry failed post-deny validation"
else
  jain_verify_locked_cargo_registry_closure \
    "$ROOT_DIR/Cargo.lock" "$HOST_CARGO_CACHE" "$cargo_deny_home" \
    || fail "security: cargo-deny mutated its exact Cargo.lock archive closure"
  jain_verify_isolated_cargo_deny_db \
    "$cargo_deny_home/advisory-dbs/advisory-db-3157b0e258782691" \
    "$RUSTSEC_DB_COMMIT" "$RUSTSEC_DB_TREE" \
    || fail "security: cargo-deny mutated its isolated advisory DB"
fi
[[ -z "$(find "$cargo_deny_home" -type l -print -quit)" ]] \
  || fail "security: isolated cargo-deny home contains a symlink"
if [[ "${JAIN_RELEASE_CI:-0}" != 1 ]]; then
  rm -rf -- "$cargo_deny_home"
  cargo_deny_home=""
else
  cargo_deny_home=""
fi

log "security: pinned offline blocking zizmor"
"$ZIZMOR_BIN" --offline --format json --no-progress .github/workflows \
  >"$artifact_root/zizmor.json"
jq -e 'type == "array" and length == 0' "$artifact_root/zizmor.json" >/dev/null

log "security: pinned update-disabled whole-repository Syft SBOM"
"$SYFT_BIN" scan dir:. --config "$SYFT_CONFIG" -q \
  --exclude './.git/**' --exclude './target/**' \
  --exclude './apps/web/node_modules/**' \
  --source-name redline-web --source-version "$(git rev-parse HEAD)" \
  --output "spdx-json=$artifact_root/redline-web.spdx.json"
jq -e '.spdxVersion | startswith("SPDX-")' \
  "$artifact_root/redline-web.spdx.json" >/dev/null

log "security: complete dev-enabled npm lock SBOM"
npm_lock_sbom="$artifact_root/npm-lock.spdx.json"
SYFT_JAVASCRIPT_INCLUDE_DEV_DEPENDENCIES=true \
  "$SYFT_BIN" scan file:apps/web/package-lock.json \
  --config "$SYFT_CONFIG" -q \
  --source-name redline-web-npm-lock --source-version "$(git rev-parse HEAD)" \
  --output "spdx-json=$npm_lock_sbom"
npm_lock_expected_purls="$artifact_root/npm-lock.expected-purls.txt"
npm_lock_actual_purls="$artifact_root/npm-lock.sbom-purls.txt"
jain_verify_npm_lock_sbom_closure \
  "$WEB_DIR/package-lock.json" "$npm_lock_sbom" \
  "$EXPECTED_NPM_PACKAGE_COUNT" redline-web-npm-lock \
  "$npm_lock_expected_purls" "$npm_lock_actual_purls" \
  || fail "security: Syft did not cover the exact complete npm lock closure"

log "security: authenticated offline Grype over the npm lock SBOM"
grype_result="$artifact_root/npm-lock.grype.json"
if ! "$GRYPE_BIN" "sbom:$npm_lock_sbom" \
  --output json --file "$grype_result" --fail-on high; then
  if [[ -f "$grype_result" ]]; then
    jq '.' "$grype_result" >&2 || true
  fi
  fail "security: Grype found a high/critical npm vulnerability or failed"
fi
jain_verify_grype_result "$grype_result" \
  || fail "security: Grype result is malformed or contains high/critical findings"
high_or_critical="$(jq -er '
  select(.matches | type == "array")
  | [.matches[]? | select(
      .vulnerability.severity == "High"
      or .vulnerability.severity == "Critical"
    )] | length
' "$grype_result")" || fail "security: Grype result is malformed"
[[ "$high_or_critical" == 0 ]] \
  || fail "security: Grype result contains a high/critical npm vulnerability"
jain_verify_grype_db_authority \
  "$GRYPE_DB_ROOT" "$GRYPE_DB_INVENTORY_SHA256" \
  || fail "security: governed Grype database changed during the scan"

package_lock_sha256="$(jain_sha256 "$WEB_DIR/package-lock.json")"
npm_lock_sbom_sha256="$(jain_sha256 "$npm_lock_sbom")"
npm_multiset_sha256="$(jain_sha256 "$npm_lock_expected_purls")"
grype_result_sha256="$(jain_sha256 "$grype_result")"
grype_db_status_sha256="$(jain_sha256 "$grype_db_status")"
repo_sbom_sha256="$(jain_sha256 "$artifact_root/redline-web.spdx.json")"
if [[ "${JAIN_RELEASE_CI:-0}" == 1 ]]; then
  cargo_package_count="$(jq -er '.package_count' \
    "$HOST_CARGO_REGISTRY/stage-receipt.json")"
else
  cargo_package_count="$(jain_locked_registry_package_records \
    "$ROOT_DIR/Cargo.lock" | wc -l)"
fi

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
  --arg grype_sha256 "$GRYPE_SHA256" \
  --arg cargo_mode "$CARGO_REGISTRY_TRUST_DOMAIN" \
  --arg cargo_source "$HOST_CARGO_REGISTRY" \
  --arg cargo_index_manifest "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" \
  --arg cargo_stage_receipt_sha256 "$cargo_stage_receipt_sha256" \
  --arg cargo_lock_closure_sha256 "$cargo_lock_closure_sha256" \
  --arg cargo_inventory_before_sha256 "$cargo_inventory_before" \
  --arg cargo_inventory_after_sha256 "$cargo_inventory_after" \
  --arg package_lock_sha256 "$package_lock_sha256" \
  --arg npm_lock_sbom_sha256 "$npm_lock_sbom_sha256" \
  --arg npm_multiset_sha256 "$npm_multiset_sha256" \
  --arg grype_db_inventory_sha256 "$GRYPE_DB_INVENTORY_SHA256" \
  --arg grype_db_status_sha256 "$grype_db_status_sha256" \
  --arg grype_result_sha256 "$grype_result_sha256" \
  --arg repo_sbom_sha256 "$repo_sbom_sha256" \
  --argjson cargo_package_count "$cargo_package_count" \
  --argjson high_or_critical "$high_or_critical" \
  --argjson npm_dependency_count "$EXPECTED_NPM_PACKAGE_COUNT" \
  '{schema_version:"redline.web.security/v2",status:"pass",network:"offline",
    commit:$commit,tree:$tree,rustsec_db_commit:$rustsec_commit,
    rustsec_db_tree:$rustsec_tree,rustsec_db_archive_sha256:$rustsec_archive_sha256,
    tool_sha256:{cargo_audit:$cargo_audit_sha256,cargo_deny:$cargo_deny_sha256,
      gitleaks:$gitleaks_sha256,zizmor:$zizmor_sha256,syft:$syft_sha256,
      grype:$grype_sha256},
    cargo:{mode:$cargo_mode,source:$cargo_source,
      stage_receipt_sha256:$cargo_stage_receipt_sha256,
      lock_closure_sha256:$cargo_lock_closure_sha256,
      index_manifest:$cargo_index_manifest,
      inventory_before_sha256:$cargo_inventory_before_sha256,
      inventory_after_sha256:$cargo_inventory_after_sha256,
      package_count:$cargo_package_count},
    javascript:{package_lock_sha256:$package_lock_sha256,
      lock_dependency_count:$npm_dependency_count,
      npm_purl_count:($npm_dependency_count + 1),
      sbom_sha256:$npm_lock_sbom_sha256,multiset_sha256:$npm_multiset_sha256,
      grype_db_inventory_sha256:$grype_db_inventory_sha256,
      grype_db_status_sha256:$grype_db_status_sha256,
      grype_result_sha256:$grype_result_sha256,fail_on:"high",
      high_or_critical:$high_or_critical},
    whole_repository_sbom:{sha256:$repo_sbom_sha256,feeds_javascript_gate:false},
    checks:["gitleaks","cargo-audit-pinned-no-fetch","cargo-deny-no-fetch",
      "zizmor-offline-strict","syft-whole-repo-update-disabled",
      "syft-npm-lock-complete-dev","grype-root-staged-offline"]}' \
  >"$artifact_root/source-evidence.json"
jq -e '
  .schema_version == "redline.web.security/v2"
  and .status == "pass" and .network == "offline"
  and .cargo.inventory_before_sha256 == .cargo.inventory_after_sha256
  and .javascript.lock_dependency_count == 376
  and .javascript.npm_purl_count == 377
  and .javascript.fail_on == "high"
  and .javascript.high_or_critical == 0
  and .whole_repository_sbom.feeds_javascript_gate == false
' "$artifact_root/source-evidence.json" >/dev/null
log "security: complete"
