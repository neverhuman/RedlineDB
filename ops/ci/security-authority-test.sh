#!/usr/bin/env bash
# Hostile fixtures for complete npm-lock SBOM coverage and governed Grype DBs.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"

SYFT_BIN="$(command -v syft)"
readonly SYFT_BIN
GRYPE_BIN="$(command -v grype)"
readonly GRYPE_BIN
readonly SYFT_SHA256="eb9714fb8e4b8f2a647e7bb312f1e0b9f83a7aa30418658bf46583cfa83d27d2"
readonly GRYPE_SHA256="ba5cdfb57056c8a68c313c8ed78e26567b310ade7b251cba03af582c3ed88672"
readonly LOCAL_GRYPE_INVENTORY="3f673a9c1e40b6181fb504d0d061973490777160acc42ccc126453fda27f2db4"

jain_verify_exact_executable syft "$SYFT_BIN" "$SYFT_SHA256"
jain_verify_exact_executable grype "$GRYPE_BIN" "$GRYPE_SHA256"

if [[ "${JAIN_RELEASE_CI:-0}" == 1 ]]; then
  grype_root="${JAIN_GRYPE_DB_ROOT:?release Grype root is required}"
  grype_inventory="${JAIN_GRYPE_DB_INVENTORY_SHA256:?release Grype inventory is required}"
  [[ "$grype_root" == /opt/jain-ci/grype-db \
    && "${GRYPE_DB_CACHE_DIR:-}" == "$grype_root" ]] \
    || fail "security-authority: release Grype authority is unbound"
else
  grype_root="/var/lib/jain-host-ci/grype-db/$LOCAL_GRYPE_INVENTORY"
  grype_inventory="$LOCAL_GRYPE_INVENTORY"
fi

jain_ci_scratch_create "$ROOT_DIR" redline-web-security-authority \
  || fail "security-authority: cannot create safe scratch directory"
tmp="$JAIN_CI_SCRATCH_PATH"
cleanup() {
  local rc=$?
  trap - EXIT
  jain_ci_scratch_remove || exit 1
  exit "$rc"
}
trap cleanup EXIT

complete_sbom="$tmp/complete.spdx.json"
default_sbom="$tmp/default.spdx.json"
SYFT_CHECK_FOR_APP_UPDATE=false \
SYFT_JAVASCRIPT_INCLUDE_DEV_DEPENDENCIES=true \
  "$SYFT_BIN" scan file:apps/web/package-lock.json \
  --config ops/ci/syft.yaml -q \
  --source-name redline-web-npm-lock --source-version fixture \
  --output "spdx-json=$complete_sbom"
jain_verify_npm_lock_sbom_closure \
  apps/web/package-lock.json "$complete_sbom" 376 redline-web-npm-lock \
  "$tmp/complete.expected" "$tmp/complete.actual" \
  || fail "security-authority: complete dev-enabled npm closure was rejected"

SYFT_CHECK_FOR_APP_UPDATE=false \
SYFT_JAVASCRIPT_INCLUDE_DEV_DEPENDENCIES=false \
  "$SYFT_BIN" scan file:apps/web/package-lock.json \
  --config ops/ci/syft.yaml -q \
  --source-name redline-web-npm-lock --source-version fixture \
  --output "spdx-json=$default_sbom"
default_npm_count="$(jq '
  [.packages[].externalRefs[]?
    | select(.referenceType == "purl")
    | .referenceLocator
    | select(startswith("pkg:npm/"))] | length
' "$default_sbom")"
[[ "$default_npm_count" -lt 377 ]] \
  || fail "security-authority: dev-disabled Syft unexpectedly covered the full lock"
if jain_verify_npm_lock_sbom_closure \
  apps/web/package-lock.json "$default_sbom" 376 redline-web-npm-lock \
  "$tmp/default.expected" "$tmp/default.actual" >/dev/null 2>&1; then
  fail "security-authority: dev-omitting Syft SBOM was accepted"
fi

jq '.name = "redline-web-whole-repository"' \
  "$complete_sbom" >"$tmp/contaminated.spdx.json"
if jain_verify_npm_lock_sbom_closure \
  apps/web/package-lock.json "$tmp/contaminated.spdx.json" \
  376 redline-web-npm-lock \
  "$tmp/contaminated.expected" "$tmp/contaminated.actual" \
  >/dev/null 2>&1; then
  fail "security-authority: whole-repository source contaminated the npm gate"
fi

jain_verify_grype_db_authority "$grype_root" "$grype_inventory" \
  || fail "security-authority: governed Grype authority was rejected"
GRYPE_CHECK_FOR_APP_UPDATE=false GRYPE_DB_AUTO_UPDATE=false \
GRYPE_DB_CACHE_DIR="$grype_root" \
  "$GRYPE_BIN" db status -o json >"$tmp/grype-status.json"
jain_verify_grype_db_status "$tmp/grype-status.json" "$grype_root" \
  || fail "security-authority: governed Grype status was rejected"

if jain_verify_grype_db_authority "$tmp/missing-db" "$grype_inventory" \
  >/dev/null 2>&1; then
  fail "security-authority: missing Grype DB was accepted"
fi
mkdir -p "$tmp/empty-db/6"
if jain_verify_grype_db_authority "$tmp/empty-db" "$grype_inventory" \
  >/dev/null 2>&1; then
  fail "security-authority: empty Grype DB was accepted"
fi
ln -s -- "$grype_root" "$tmp/symlink-db"
if jain_verify_grype_db_authority "$tmp/symlink-db" "$grype_inventory" \
  >/dev/null 2>&1; then
  fail "security-authority: symlinked Grype DB was accepted"
fi
rm -- "$tmp/symlink-db"

mkdir -p "$tmp/closed-db/6"
printf '{}\n' >"$tmp/closed-db/6/import.json"
printf 'fixture\n' >"$tmp/closed-db/6/vulnerability.db"
closed_inventory="$(jain_grype_db_inventory_sha256 "$tmp/closed-db")"
printf 'extra\n' >"$tmp/closed-db/extra"
if jain_grype_db_inventory_sha256 "$tmp/closed-db" >/dev/null 2>&1; then
  fail "security-authority: extra Grype DB node was accepted"
fi
rm -- "$tmp/closed-db/extra"
[[ "$(jain_grype_db_inventory_sha256 "$tmp/closed-db")" == "$closed_inventory" ]] \
  || fail "security-authority: closed fixture inventory is unstable"
if jain_verify_grype_db_authority "$grype_root" \
  0000000000000000000000000000000000000000000000000000000000000000 \
  >/dev/null 2>&1; then
  fail "security-authority: wrong Grype inventory was accepted"
fi

jq -n --arg root "$grype_root" \
  '{valid:true,schemaVersion:"v6.1.9",path:($root + "/other.db")}' \
  >"$tmp/unbound-status.json"
if jain_verify_grype_db_status "$tmp/unbound-status.json" "$grype_root" \
  >/dev/null 2>&1; then
  fail "security-authority: unbound Grype status was accepted"
fi
jq -n --arg root "$grype_root" \
  '{valid:false,schemaVersion:"v6.1.9",path:($root + "/6/vulnerability.db")}' \
  >"$tmp/invalid-status.json"
if jain_verify_grype_db_status "$tmp/invalid-status.json" "$grype_root" \
  >/dev/null 2>&1; then
  fail "security-authority: invalid Grype status was accepted"
fi
jq -n --arg root "$grype_root" \
  '{valid:true,schemaVersion:"v5.0.0",path:($root + "/6/vulnerability.db")}' \
  >"$tmp/wrong-schema-status.json"
if jain_verify_grype_db_status "$tmp/wrong-schema-status.json" "$grype_root" \
  >/dev/null 2>&1; then
  fail "security-authority: wrong Grype schema was accepted"
fi

jq -n '{matches:[]}' >"$tmp/clean-grype.json"
jain_verify_grype_result "$tmp/clean-grype.json" \
  || fail "security-authority: clean Grype result was rejected"
jq -n \
  '{matches:[{vulnerability:{severity:"High"}}]}' >"$tmp/high-grype.json"
if jain_verify_grype_result "$tmp/high-grype.json" >/dev/null 2>&1; then
  fail "security-authority: high Grype finding was accepted"
fi

log "security-authority: complete npm closure and hostile Grype fixtures passed"
