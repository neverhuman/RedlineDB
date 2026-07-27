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

readonly CARGO_DENY_BIN="/home/ubuntu/.cargo/bin/cargo-deny"
readonly CARGO_DENY_DB="/home/ubuntu/.cargo/advisory-dbs/advisory-db-3157b0e258782691"
readonly RUSTSEC_DB_COMMIT="9f3e138091487e69144f536d36976e427a7a3307"
readonly RUSTSEC_DB_TREE="c33f1047906505cabcec7e21f2d99db5c6de8852"
readonly HOST_CARGO_INDEX="/home/ubuntu/.cargo/registry/index/index.crates.io-1949cf8c6b5b557f"
readonly LOCK_CLOSURE_INDEX_MANIFEST_SHA256="aac1119a086ca336165f570d5794853f0dfedb444ed4300cf13a77bac170ffad"
readonly HOST_CARGO_CACHE_PARENT="/home/ubuntu/.cargo/registry/cache"
readonly HOST_CARGO_CACHE="${HOST_CARGO_CACHE_PARENT}/index.crates.io-1949cf8c6b5b557f"

jain_ci_scratch_create "$ROOT_DIR" redline-web-security-no-index \
  || fail "unable to create a custody-safe in-repository scratch directory"
tmp="$JAIN_CI_SCRATCH_PATH"
cleanup() {
  local rc=$?
  trap - EXIT
  jain_ci_scratch_remove || exit 1
  exit "$rc"
}
trap cleanup EXIT

# Metadata is produced from the committed lockfile only.
cargo_cache_home="${CARGO_HOME:-${HOME}/.cargo}"
metadata="$tmp/cargo-metadata.json"
CARGO_HOME="$cargo_cache_home" cargo metadata --locked --offline --format-version 1 \
  >"$metadata" || fail "no-index test: locked offline cargo metadata failed"

# Hostile home: the Cargo.lock closure and the pinned advisory DB, nothing else.
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

# Add the same receipt/lock-source binding supplied by cargo-cache-stage so the
# product-side staged-registry validator is exercised without another copy.
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
jain_verify_staged_cargo_registry \
  "$ROOT_DIR/Cargo.lock" "$registry" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" \
  || fail "no-index test: valid staged Cargo receipt/closure was rejected"
staged_inventory_before="$(jain_staged_cargo_registry_inventory_sha256 "$registry")"

# The seeded index must be strictly smaller than the host's: closure, not copy.
host_entries="$(find "$HOST_CARGO_INDEX/.cache" -type f | wc -l)"
seeded_entries="$(find "$deny_home/registry/index"/*/.cache -type f | wc -l)"
[[ "$seeded_entries" -gt 0 && "$seeded_entries" -lt "$host_entries" ]] \
  || fail "no-index test: seeded index is empty or is a full copy ($seeded_entries vs $host_entries)"

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
  "$deny_home/registry/index/$(basename -- "$HOST_CARGO_INDEX")" \
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
seeded_index="$deny_home/registry/index/$(basename -- "$HOST_CARGO_INDEX")"
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
cp -- "$registry/stage-receipt.json" "$tmp/stage-receipt.good.json"
jq '.packages += [.packages[0]] | .package_count += 1' \
  "$tmp/stage-receipt.good.json" >"$registry/stage-receipt.json"
if jain_verify_staged_cargo_registry \
  "$ROOT_DIR/Cargo.lock" "$registry" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" >/dev/null 2>&1; then
  fail "no-index test: staged registry accepted a duplicate receipt package"
fi
cp -- "$tmp/stage-receipt.good.json" "$registry/stage-receipt.json"
jain_verify_staged_cargo_registry \
  "$ROOT_DIR/Cargo.lock" "$registry" \
  "$LOCK_CLOSURE_INDEX_MANIFEST_SHA256" \
  || fail "no-index test: staged registry did not recover after hostile fixtures"

# A tampered selected entry must be rejected rather than silently accepted.
tamper_home="$tmp/tamper-home"
mkdir -p "$tamper_home"
if jain_seed_locked_cargo_registry_index \
  "$ROOT_DIR/Cargo.lock" "$HOST_CARGO_INDEX" "$tamper_home" \
  0000000000000000000000000000000000000000000000000000000000000000 \
  >/dev/null 2>&1; then
  fail "no-index test: a wrong closure manifest was accepted"
fi

# Yanked detection must be live, not silently degraded.
! grep -q "index-failure" "$deny_log" \
  || fail "no-index test: cargo-deny could not check for yanked crates"

log "security-no-index: proof ok — cargo-deny completes offline from the lock closure with yanked checking live (index entries=$seeded_entries of $host_entries)"
