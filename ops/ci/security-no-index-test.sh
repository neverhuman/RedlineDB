#!/usr/bin/env bash
# Hostile proof that the security lane's cargo-deny stage needs only the
# Cargo.lock closure of the crates.io index -- never the whole developer cache.
#
# The lane used to seed an isolated index copied WHOLESALE from the developer
# cache at ~/.cargo and verify it by whole-directory digest. That cache is shared
# and appendable: ordinary crate resolution anywhere on the host mutates it, so
# the pin fired on correct behaviour rather than on tampering, and no value for it
# could stay true. The developer cache is therefore not an authoritative custody
# object, and the lane now reads only the closure of the committed lockfile.
#
# Dropping the index entirely is NOT an option and this test is why: cargo-deny
# needs index entries to answer whether a locked version is yanked, and without
# them it emits error[index-failure] and the advisories check fails. So the proof
# is that the LOCKFILE CLOSURE is sufficient -- cargo-deny completes clean against
# a dead network with a strict subset of the host index, with yanked checking
# still live rather than silently degraded.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"

readonly LOCAL_RUSTSEC_DB_COMMIT="6e3286f4efa8c142fb33e5ea4342c8db6693cf34"
readonly LOCAL_RUSTSEC_DB_TREE="d12220aff0053a035739bec6e64aefbaafbf01a3"
readonly GOVERNED_INDEX_MANIFEST_SHA256="45169acea56070f13e18ce19f2f4d3ec86abaec0cf47365b9f27b31505b2abb3"
readonly DEVELOPER_INDEX_MANIFEST_SHA256="aac1119a086ca336165f570d5794853f0dfedb444ed4300cf13a77bac170ffad"

CARGO_DENY_BIN="$(command -v cargo-deny 2>/dev/null || true)"
if [[ "${JAIN_RELEASE_CI:-0}" == 1 ]]; then
  [[ -n "$CARGO_DENY_BIN" ]] \
    || fail "no-index test: release cargo-deny shim is unavailable"
  rustsec_identity="$(jain_resolve_release_rustsec_authority \
    "${JAIN_RUSTSEC_ADVISORY_SOURCE:-}" "${JAIN_PINNED_ADVISORY_DB:-}" \
    "${JAIN_ADVISORY_DB:-}" "${JAIN_CARGO_DENY_ADVISORY_DB:-}" \
    "${JAIN_PINNED_ADVISORY_COMMIT:-}" \
    "$LOCAL_RUSTSEC_DB_COMMIT" "$LOCAL_RUSTSEC_DB_TREE")" \
    || fail "no-index test: release RustSec authority is invalid"
  SOURCE_CARGO_REGISTRY="${CARGO_HOME:?release CARGO_HOME is required}/registry"
  LOCK_CLOSURE_INDEX_MANIFEST_SHA256="$GOVERNED_INDEX_MANIFEST_SHA256"
  cargo_cache_home="$CARGO_HOME"
  release_mode=1
else
  [[ -n "$CARGO_DENY_BIN" ]] \
    || CARGO_DENY_BIN="${HOME:?developer HOME is required}/.cargo/bin/cargo-deny"
  RUSTSEC_DB="$(jain_first_present_path \
    "${JAIN_RUSTSEC_ADVISORY_SOURCE:-}" "${JAIN_PINNED_ADVISORY_DB:-}" \
    "${JAIN_ADVISORY_DB:-}" \
    "${HOME:?developer HOME is required}/.cargo/advisory-db" || true)"
  CARGO_DENY_DB="$(jain_first_present_path \
    "${JAIN_CARGO_DENY_ADVISORY_DB:-}" \
    "${HOME:?developer HOME is required}/.cargo/advisory-dbs/advisory-db-3157b0e258782691" \
    || true)"
  rustsec_identity="$(jain_resolve_rustsec_authority \
    local "$RUSTSEC_DB" "$CARGO_DENY_DB" "" \
    "$LOCAL_RUSTSEC_DB_COMMIT" "$LOCAL_RUSTSEC_DB_TREE")" \
    || fail "no-index test: local RustSec authority is invalid"
  SOURCE_CARGO_REGISTRY="${HOME:?developer HOME is required}/.cargo/registry"
  LOCK_CLOSURE_INDEX_MANIFEST_SHA256="$DEVELOPER_INDEX_MANIFEST_SHA256"
  cargo_cache_home="${CARGO_HOME:-${HOME:?developer HOME is required}/.cargo}"
  release_mode=0
fi
IFS=$'\t' read -r RUSTSEC_DB_COMMIT RUSTSEC_DB_TREE <<<"$rustsec_identity"
readonly CARGO_DENY_BIN RUSTSEC_DB_COMMIT RUSTSEC_DB_TREE
readonly SOURCE_CARGO_REGISTRY LOCK_CLOSURE_INDEX_MANIFEST_SHA256
readonly release_mode cargo_cache_home

if [[ "$release_mode" == 1 ]]; then
  jain_verify_staged_cargo_registry \
    "$ROOT_DIR/Cargo.lock" "$SOURCE_CARGO_REGISTRY" \
    "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" \
    || fail "no-index test: release root-staged Cargo authority is invalid"
fi
mapfile -t cargo_cache_children < <(
  find "$SOURCE_CARGO_REGISTRY/cache" -mindepth 1 -maxdepth 1 -type d -print
)
mapfile -t cargo_index_children < <(
  find "$SOURCE_CARGO_REGISTRY/index" -mindepth 1 -maxdepth 1 -type d -print
)
[[ "${#cargo_cache_children[@]}" == 1 \
  && "${#cargo_index_children[@]}" == 1 \
  && "$(basename -- "${cargo_cache_children[0]}")" \
    == "$(basename -- "${cargo_index_children[0]}")" ]] \
  || fail "no-index test: Cargo source must have one matching cache/index authority"
HOST_CARGO_CACHE="${cargo_cache_children[0]}"
HOST_CARGO_INDEX="${cargo_index_children[0]}"
HOST_CARGO_CACHE_PARENT="${SOURCE_CARGO_REGISTRY}/cache"
readonly HOST_CARGO_CACHE HOST_CARGO_INDEX HOST_CARGO_CACHE_PARENT

jain_ci_scratch_create "$ROOT_DIR" redline-web-security-no-index \
  || fail "unable to create a custody-safe in-repository scratch directory"
tmp="$JAIN_CI_SCRATCH_PATH"
registry=""
receipt_backup=""
seeded_index=""
seeded_cache=""
cleanup() {
  local rc=$?
  trap - EXIT
  if [[ -n "$seeded_index" ]]; then
    rm -f -- "$seeded_index/.cache/post-seed-extra"
  fi
  if [[ -n "$seeded_cache" ]]; then
    rm -f -- "$seeded_cache/post-seed-extra.crate"
  fi
  if [[ -n "$registry" && -f "$receipt_backup" ]]; then
    cp -- "$receipt_backup" "$registry/stage-receipt.json"
  fi
  jain_ci_scratch_remove || exit 1
  exit "$rc"
}
trap cleanup EXIT

# Metadata is produced from the committed lockfile only.
metadata="$tmp/cargo-metadata.json"
CARGO_HOME="$cargo_cache_home" cargo metadata --locked --offline --format-version 1 \
  >"$metadata" || fail "no-index test: locked offline cargo metadata failed"

# Release executes directly against the root-created fresh Cargo home. Its
# standalone RustSec snapshots intentionally have no origin/FETCH_HEAD, so the
# local clone/seeder path is both unnecessary and invalid there. Developer mode
# retains the explicit local closure construction and lineage validator.
if [[ "$release_mode" == 1 ]]; then
  deny_home="$CARGO_HOME"
  registry="$SOURCE_CARGO_REGISTRY"
else
  deny_home="$tmp/cargo-deny-home"
  mkdir -p "$deny_home"
  jain_seed_locked_cargo_registry_index \
    "$ROOT_DIR/Cargo.lock" "$HOST_CARGO_INDEX" "$deny_home" \
    "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" \
    || fail "no-index test: lock-closure crates.io index seed failed"
  jain_seed_locked_cargo_registry_closure \
    "$ROOT_DIR/Cargo.lock" "$HOST_CARGO_CACHE_PARENT" "$HOST_CARGO_CACHE" \
    "$deny_home" \
    || fail "no-index test: exact Cargo.lock registry closure seed failed"
  jain_seed_cargo_deny_advisory_db \
    "$CARGO_DENY_DB" "$deny_home" "$RUSTSEC_DB_COMMIT" "$RUSTSEC_DB_TREE" \
    || fail "no-index test: exact isolated advisory DB seed failed"

  # Mirror the receipt/lock-source binding supplied by root cargo-cache-stage.
  registry="$deny_home/registry"
  records="$tmp/locked-registry-records.tsv"
  jain_locked_registry_package_records "$ROOT_DIR/Cargo.lock" >"$records"
  lock_sha256="$(jain_sha256 "$ROOT_DIR/Cargo.lock")"
  jq -Rn --arg lock_sha256 "$lock_sha256" '
    [inputs | split("\t")
      | {name:.[0],version:.[1],checksum:.[2]}] as $packages
    | {schema_version:"jain.locked-cargo-cache/v2",
        lock_count:1,lock_sha256s:[$lock_sha256],
        package_count:($packages | length),packages:$packages,
        governed_git_repositories:[]}
  ' <"$records" >"$registry/stage-receipt.json"
  jq -n --arg lock_sha256 "$lock_sha256" '
    {schema_version:"jain.cargo-lock-source-closure/v1",
      lock_count:1,lock_sha256s:[$lock_sha256],sources:[]}
  ' >"$registry/lock-source-closure.json"
fi

jain_verify_staged_cargo_registry \
  "$ROOT_DIR/Cargo.lock" "$registry" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" \
  || fail "no-index test: valid staged Cargo receipt/closure was rejected"
staged_inventory_before="$(jain_staged_cargo_registry_inventory_sha256 "$registry")"

# Missing receipts/closures and symlinked staged nodes must fail in both modes.
stage_hostile="$tmp/stage-authority-hostile"
cp -a --reflink=auto -- "$registry" "$stage_hostile"
rm -- "$stage_hostile/stage-receipt.json"
if jain_verify_staged_cargo_registry \
  "$ROOT_DIR/Cargo.lock" "$stage_hostile" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" >/dev/null 2>&1; then
  fail "no-index test: staged registry without a receipt was accepted"
fi
cp -- "$registry/stage-receipt.json" "$stage_hostile/stage-receipt.json"
rm -- "$stage_hostile/lock-source-closure.json"
if jain_verify_staged_cargo_registry \
  "$ROOT_DIR/Cargo.lock" "$stage_hostile" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" >/dev/null 2>&1; then
  fail "no-index test: staged registry without a lock closure was accepted"
fi
cp -- "$registry/lock-source-closure.json" \
  "$stage_hostile/lock-source-closure.json"
mv -- "$stage_hostile/index" "$stage_hostile/index.physical"
ln -s -- index.physical "$stage_hostile/index"
if jain_verify_staged_cargo_registry \
  "$ROOT_DIR/Cargo.lock" "$stage_hostile" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" >/dev/null 2>&1; then
  fail "no-index test: staged registry with a symlinked index was accepted"
fi
rm -- "$stage_hostile/index"
mv -- "$stage_hostile/index.physical" "$stage_hostile/index"
mv -- "$stage_hostile/cache" "$stage_hostile/cache.physical"
ln -s -- cache.physical "$stage_hostile/cache"
if jain_verify_staged_cargo_registry \
  "$ROOT_DIR/Cargo.lock" "$stage_hostile" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" >/dev/null 2>&1; then
  fail "no-index test: staged registry with a symlinked cache was accepted"
fi

# A developer source can contain more index entries than the selected closure.
# A release source is already the exact root-staged closure, so equality is
# required and no hidden full-host inventory is consulted.
host_entries="$(find "$HOST_CARGO_INDEX/.cache" -type f | wc -l)"
seeded_entries="$(find "$registry/index"/*/.cache -type f | wc -l)"
[[ "$seeded_entries" -gt 0 && "$seeded_entries" -le "$host_entries" ]] \
  || fail "no-index test: staged index exceeds its source authority"
if [[ "$release_mode" == 1 ]]; then
  [[ "$seeded_entries" == "$host_entries" ]] \
    || fail "no-index test: release index differs from its exact root-staged closure"
else
  [[ "$seeded_entries" -lt "$host_entries" ]] \
    || fail "no-index test: local closure is not a strict subset of its shared source"
fi

# Dead network: an unroutable proxy plus offline/disable-fetch. If cargo-deny
# needed the index it would try to fetch here and fail.
log "security-no-index: cargo-deny from the bound lock closure, no index, dead network"
deny_log="$tmp/cargo-deny.log"
if ! env CARGO_HOME="$deny_home" \
  CARGO_NET_OFFLINE=true \
  http_proxy="http://127.0.0.1:1" https_proxy="http://127.0.0.1:1" \
  all_proxy="http://127.0.0.1:1" \
  "$CARGO_DENY_BIN" check --metadata-path "$metadata" --disable-fetch \
  --deny warnings >"$deny_log" 2>&1; then
  cat "$deny_log" >&2
  fail "no-index test: cargo-deny could not complete from the lock-closure index"
fi
jain_verify_cargo_deny_clean_log "$deny_log" || {
  cat "$deny_log" >&2
  fail "no-index test: cargo-deny output was not the exact passing summary"
}

# Content custody must still hold after the governed decision.
jain_verify_locked_cargo_registry_index \
  "$ROOT_DIR/Cargo.lock" \
  "$registry/index/$(basename -- "$HOST_CARGO_INDEX")" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" \
  || fail "no-index test: isolated index changed while cargo-deny ran"
jain_verify_staged_cargo_registry \
  "$ROOT_DIR/Cargo.lock" "$registry" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" \
  || fail "no-index test: staged Cargo registry changed while cargo-deny ran"
[[ "$(jain_staged_cargo_registry_inventory_sha256 "$registry")" \
  == "$staged_inventory_before" ]] \
  || fail "no-index test: staged Cargo inventory changed while cargo-deny ran"

# A physical file added after seeding must be rejected even though it is outside
# the selected-content manifest. This closes the exact-set/post-run seam.
seeded_index="$registry/index/$(basename -- "$HOST_CARGO_INDEX")"
printf 'hostile extra\n' >"$seeded_index/.cache/post-seed-extra"
if jain_verify_locked_cargo_registry_index \
  "$ROOT_DIR/Cargo.lock" "$seeded_index" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" >/dev/null 2>&1; then
  fail "no-index test: post-seed extra index file was accepted"
fi
if jain_verify_staged_cargo_registry \
  "$ROOT_DIR/Cargo.lock" "$registry" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" >/dev/null 2>&1; then
  fail "no-index test: staged registry accepted an extra index file"
fi
rm -- "$seeded_index/.cache/post-seed-extra"
jain_verify_locked_cargo_registry_index \
  "$ROOT_DIR/Cargo.lock" "$seeded_index" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" \
  || fail "no-index test: exact index did not recover after hostile fixture"

# Extra archives and duplicate receipt packages must also fail closed.
seeded_cache="$registry/cache/$(basename -- "$HOST_CARGO_CACHE")"
printf 'hostile archive\n' >"$seeded_cache/post-seed-extra.crate"
if jain_verify_staged_cargo_registry \
  "$ROOT_DIR/Cargo.lock" "$registry" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" >/dev/null 2>&1; then
  fail "no-index test: staged registry accepted an extra archive"
fi
rm -- "$seeded_cache/post-seed-extra.crate"
receipt_backup="$tmp/stage-receipt.good.json"
cp -- "$registry/stage-receipt.json" "$receipt_backup"
jq '.packages += [.packages[0]] | .package_count += 1' \
  "$receipt_backup" >"$registry/stage-receipt.json"
if jain_verify_staged_cargo_registry \
  "$ROOT_DIR/Cargo.lock" "$registry" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" >/dev/null 2>&1; then
  fail "no-index test: staged registry accepted a duplicate receipt package"
fi
cp -- "$receipt_backup" "$registry/stage-receipt.json"
jain_verify_staged_cargo_registry \
  "$ROOT_DIR/Cargo.lock" "$registry" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" \
  || fail "no-index test: staged registry did not recover after hostile fixtures"

# A wrong selected-entry manifest must be rejected without constructing a new
# registry from hidden host state.
if jain_verify_staged_cargo_registry \
  "$ROOT_DIR/Cargo.lock" "$registry" \
  0000000000000000000000000000000000000000000000000000000000000000 \
  >/dev/null 2>&1; then
  fail "no-index test: a wrong closure manifest was accepted"
fi

# Yanked detection must be live, not silently degraded.
! grep -q "index-failure" "$deny_log" \
  || fail "no-index test: cargo-deny could not check for yanked crates"

log "security-no-index: proof ok — cargo-deny completes offline from the lock closure with yanked checking live (index entries=$seeded_entries of $host_entries)"
