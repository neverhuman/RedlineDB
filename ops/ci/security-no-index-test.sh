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
  || fail "no-index test: lock-closure crates.io index seed failed"
jain_seed_locked_cargo_registry_closure \
  "$ROOT_DIR/Cargo.lock" "$HOST_CARGO_CACHE_PARENT" "$HOST_CARGO_CACHE" \
  "$deny_home" \
  || fail "no-index test: exact Cargo.lock registry closure seed failed"
jain_seed_cargo_deny_advisory_db \
  "$CARGO_DENY_DB" "$deny_home" "$RUSTSEC_DB_COMMIT" "$RUSTSEC_DB_TREE" \
  || fail "no-index test: exact isolated advisory DB seed failed"

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

# Yanked detection must be live, not silently degraded.
! grep -q "index-failure" "$deny_log" \
  || fail "no-index test: cargo-deny could not check for yanked crates"

log "security-no-index: proof ok — cargo-deny completes offline from the lock closure with yanked checking live (index entries=$seeded_entries of $host_entries)"
