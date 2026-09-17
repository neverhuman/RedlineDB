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

rustsec_fixture="$tmp/rustsec"
rustsec_source="$rustsec_fixture/source"
mkdir -p "$rustsec_source"
git -C "$rustsec_source" init -q -b main
git -C "$rustsec_source" config user.name 'Redline Web CI Fixture'
git -C "$rustsec_source" config user.email 'redline-web-ci@example.invalid'
printf 'first RustSec fixture\n' >"$rustsec_source/README.md"
git -C "$rustsec_source" add README.md
git -C "$rustsec_source" commit -q -m 'first RustSec fixture'
rustsec_first_commit="$(git -C "$rustsec_source" rev-parse HEAD)"
rustsec_first_tree="$(git -C "$rustsec_source" rev-parse 'HEAD^{tree}')"
printf 'second RustSec fixture\n' >>"$rustsec_source/README.md"
git -C "$rustsec_source" add README.md
git -C "$rustsec_source" commit -q -m 'second RustSec fixture'
rustsec_commit="$(git -C "$rustsec_source" rev-parse HEAD)"
rustsec_tree="$(git -C "$rustsec_source" rev-parse 'HEAD^{tree}')"
git -C "$rustsec_source" remote add origin "$rustsec_source"
git -C "$rustsec_source" fetch -q --no-tags origin refs/heads/main

new_rustsec_snapshot() {
  local destination="$1" commit="$2"
  git clone -q --no-local --no-tags "$rustsec_source" "$destination"
  git -C "$destination" checkout -q --detach "$commit"
}

expect_rustsec_rejected() {
  local label="$1"
  shift
  if jain_resolve_rustsec_authority "$@" \
    >"$rustsec_fixture/$label.stdout" 2>"$rustsec_fixture/$label.stderr"; then
    fail "security-authority: hostile RustSec fixture was accepted: $label"
  fi
}

expect_release_exports_rejected() {
  local label="$1"
  shift
  if jain_resolve_release_rustsec_authority "$@" \
    >"$rustsec_fixture/$label.stdout" 2>"$rustsec_fixture/$label.stderr"; then
    fail "security-authority: hostile release RustSec exports were accepted: $label"
  fi
}

release_audit="$rustsec_fixture/release-audit"
release_deny="$rustsec_fixture/release-deny"
new_rustsec_snapshot "$release_audit" "$rustsec_commit"
new_rustsec_snapshot "$release_deny" "$rustsec_commit"
git -C "$release_audit" remote remove origin
git -C "$release_deny" remote remove origin
rm -f -- "$release_audit/.git/FETCH_HEAD" "$release_deny/.git/FETCH_HEAD"
rustsec_identity="$(jain_resolve_rustsec_authority \
  release "$release_audit" "$release_deny" "$rustsec_commit" \
  "$rustsec_first_commit" "$rustsec_first_tree")" \
  || fail "security-authority: valid standalone RustSec pair was rejected"
[[ "$rustsec_identity" == "$rustsec_commit"$'\t'"$rustsec_tree" ]] \
  || fail "security-authority: release RustSec identity was not commit/tree bound"
rustsec_identity="$(jain_resolve_release_rustsec_authority \
  "$release_audit" "$release_audit" "$release_audit" "$release_deny" \
  "$rustsec_commit" "$rustsec_first_commit" "$rustsec_first_tree")" \
  || fail "security-authority: valid release RustSec exports were rejected"
[[ "$rustsec_identity" == "$rustsec_commit"$'\t'"$rustsec_tree" ]] \
  || fail "security-authority: release exports were not commit/tree bound"

expect_rustsec_rejected release-missing-commit \
  release "$release_audit" "$release_deny" "" \
  "$rustsec_first_commit" "$rustsec_first_tree"
expect_rustsec_rejected release-malformed-commit \
  release "$release_audit" "$release_deny" ABCDEF \
  "$rustsec_first_commit" "$rustsec_first_tree"
expect_rustsec_rejected release-wrong-existing-commit \
  release "$release_audit" "$release_deny" "$rustsec_first_commit" \
  "$rustsec_first_commit" "$rustsec_first_tree"

git -C "$release_deny" checkout -q --detach "$rustsec_first_commit"
expect_rustsec_rejected release-audit-deny-mismatch \
  release "$release_audit" "$release_deny" "$rustsec_commit" \
  "$rustsec_first_commit" "$rustsec_first_tree"
git -C "$release_deny" checkout -q --detach "$rustsec_commit"
printf 'hostile dirty tree\n' >>"$release_audit/README.md"
expect_rustsec_rejected release-dirty-tree \
  release "$release_audit" "$release_deny" "$rustsec_commit" \
  "$rustsec_first_commit" "$rustsec_first_tree"
git -C "$release_audit" checkout -q -- README.md
ln -s -- "$release_deny" "$rustsec_fixture/linked-release-db"
expect_rustsec_rejected release-symlink-db \
  release "$rustsec_fixture/linked-release-db" "$release_deny" \
  "$rustsec_commit" "$rustsec_first_commit" "$rustsec_first_tree"
expect_rustsec_rejected release-missing-db \
  release "$rustsec_fixture/missing-release-db" "$release_deny" \
  "$rustsec_commit" "$rustsec_first_commit" "$rustsec_first_tree"
expect_rustsec_rejected release-aliased-db \
  release "$release_audit" "$release_audit" "$rustsec_commit" \
  "$rustsec_first_commit" "$rustsec_first_tree"
expect_release_exports_rejected release-missing-source-export \
  "" "$release_audit" "$release_audit" "$release_deny" \
  "$rustsec_commit" "$rustsec_first_commit" "$rustsec_first_tree"
expect_release_exports_rejected release-missing-pinned-export \
  "$release_audit" "" "$release_audit" "$release_deny" \
  "$rustsec_commit" "$rustsec_first_commit" "$rustsec_first_tree"
expect_release_exports_rejected release-missing-advisory-export \
  "$release_audit" "$release_audit" "" "$release_deny" \
  "$rustsec_commit" "$rustsec_first_commit" "$rustsec_first_tree"
expect_release_exports_rejected release-mismatched-source-export \
  "$release_deny" "$release_audit" "$release_audit" "$release_deny" \
  "$rustsec_commit" "$rustsec_first_commit" "$rustsec_first_tree"
expect_release_exports_rejected release-mismatched-advisory-export \
  "$release_audit" "$release_audit" "$release_deny" "$release_deny" \
  "$rustsec_commit" "$rustsec_first_commit" "$rustsec_first_tree"

release_git_link="$rustsec_fixture/release-git-link"
new_rustsec_snapshot "$release_git_link" "$rustsec_commit"
mv -- "$release_git_link/.git" "$release_git_link/git-metadata"
ln -s -- git-metadata "$release_git_link/.git"
expect_rustsec_rejected release-symlink-git \
  release "$release_git_link" "$release_deny" "$rustsec_commit" \
  "$rustsec_first_commit" "$rustsec_first_tree"

local_deny="$rustsec_fixture/local-deny"
new_rustsec_snapshot "$local_deny" "$rustsec_commit"
git -C "$local_deny" checkout -q main
git -C "$local_deny" fetch -q --no-tags origin refs/heads/main
printf 'foreign local checkout state\n' >"$rustsec_source/FOREIGN.md"
rustsec_identity="$(jain_resolve_rustsec_authority \
  local "$rustsec_source" "$local_deny" \
  0000000000000000000000000000000000000000 \
  "$rustsec_first_commit" "$rustsec_first_tree")" \
  || fail "security-authority: advanced local RustSec source was rejected"
[[ "$rustsec_identity" == "$rustsec_first_commit"$'\t'"$rustsec_first_tree" ]] \
  || fail "security-authority: hostile release export influenced local RustSec"
rm -- "$rustsec_source/FOREIGN.md"
expect_rustsec_rejected local-wrong-commit \
  local "$rustsec_source" "$local_deny" "" \
  0000000000000000000000000000000000000000 "$rustsec_first_tree"
expect_rustsec_rejected local-wrong-tree \
  local "$rustsec_source" "$local_deny" "" \
  "$rustsec_first_commit" 0000000000000000000000000000000000000000

local_seed_home="$rustsec_fixture/local-seed-home"
mkdir "$local_seed_home"
jain_seed_cargo_deny_advisory_db \
  "$local_deny" "$local_seed_home" "$rustsec_first_commit" "$rustsec_first_tree" \
  || fail "security-authority: advanced local cargo-deny lineage was rejected"

git -C "$rustsec_source" checkout -q --orphan hostile-off-lineage
printf 'off-lineage RustSec fixture\n' >"$rustsec_source/OFF_LINEAGE.md"
git -C "$rustsec_source" add OFF_LINEAGE.md
git -C "$rustsec_source" commit -q -m 'off-lineage RustSec fixture'
off_lineage_commit="$(git -C "$rustsec_source" rev-parse HEAD)"
off_lineage_tree="$(git -C "$rustsec_source" rev-parse 'HEAD^{tree}')"
git -C "$rustsec_source" checkout -q main
off_lineage_home="$rustsec_fixture/off-lineage-home"
mkdir "$off_lineage_home"
if jain_seed_cargo_deny_advisory_db \
  "$rustsec_source" "$off_lineage_home" \
  "$off_lineage_commit" "$off_lineage_tree" \
  >"$rustsec_fixture/off-lineage.stdout" \
  2>"$rustsec_fixture/off-lineage.stderr"; then
  fail "security-authority: off-lineage local RustSec commit was accepted"
fi

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
