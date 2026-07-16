#!/usr/bin/env bash
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

tmp="$(mktemp -d /tmp/redline-web-cargo-deny-cache.XXXXXX)"
cleanup() {
  local rc=$?
  rm -rf -- "$tmp"
  return "$rc"
}
trap cleanup EXIT

case_root="$tmp/case"
cache_parent="$case_root/cache"
fixed_cache="$cache_parent/index.crates.io-1949cf8c6b5b557f"
cargo_home="$case_root/cargo-home"
lock_file="$case_root/Cargo.lock"

reset_fixture() {
  local valuable_checksum wincon_checksum
  rm -rf -- "$case_root"
  mkdir -p "$fixed_cache" "$cargo_home"
  printf 'valuable fixture archive\n' >"$fixed_cache/valuable-0.1.1.crate"
  printf 'anstyle-wincon fixture archive\n' >"$fixed_cache/anstyle-wincon-3.0.11.crate"
  valuable_checksum="$(jain_sha256 "$fixed_cache/valuable-0.1.1.crate")"
  wincon_checksum="$(jain_sha256 "$fixed_cache/anstyle-wincon-3.0.11.crate")"
  cat >"$lock_file" <<EOF
version = 3

[[package]]
name = "anstyle-wincon"
version = "3.0.11"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "$wincon_checksum"

[[package]]
name = "valuable"
version = "0.1.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "$valuable_checksum"
EOF
}

expect_rejected() {
  local label="$1" expected="$2"
  local stderr_file="$case_root/$label.stderr"
  if jain_seed_locked_cargo_archives \
    "$lock_file" "$cache_parent" "$fixed_cache" "$cargo_home" \
    valuable 0.1.1 anstyle-wincon 3.0.11 \
    >"$case_root/$label.stdout" 2>"$stderr_file"; then
    printf '%s fixture was accepted\n' "$label" >&2
    exit 1
  fi
  grep -F -- "$expected" "$stderr_file" >/dev/null || {
    printf '%s fixture failed without expected diagnostic: %s\n' "$label" "$expected" >&2
    cat "$stderr_file" >&2
    exit 1
  }
}

reset_fixture
[[ -z "$(find "$cargo_home" -mindepth 1 -print -quit)" ]]
jain_seed_locked_cargo_archives \
  "$lock_file" "$cache_parent" "$fixed_cache" "$cargo_home" \
  valuable 0.1.1 anstyle-wincon 3.0.11
destination="$cargo_home/registry/cache/$(basename "$fixed_cache")"
[[ "$(jain_sha256 "$destination/valuable-0.1.1.crate")" \
  == "$(jain_sha256 "$fixed_cache/valuable-0.1.1.crate")" ]]
[[ "$(jain_sha256 "$destination/anstyle-wincon-3.0.11.crate")" \
  == "$(jain_sha256 "$fixed_cache/anstyle-wincon-3.0.11.crate")" ]]

reset_fixture
rm -- "$fixed_cache/valuable-0.1.1.crate"
expect_rejected missing 'locked crate archive is missing'

reset_fixture
rm -- "$fixed_cache/anstyle-wincon-3.0.11.crate"
ln -s /dev/null "$fixed_cache/anstyle-wincon-3.0.11.crate"
expect_rejected symlink 'locked crate archive must be a regular non-symlink'

reset_fixture
printf 'corruption\n' >>"$fixed_cache/valuable-0.1.1.crate"
expect_rejected wrong-digest 'locked crate archive digest does not match Cargo.lock'

reset_fixture
duplicate_cache="$cache_parent/index.crates.io-duplicate"
mkdir "$duplicate_cache"
cp -- "$fixed_cache/valuable-0.1.1.crate" "$duplicate_cache/valuable-0.1.1.crate"
expect_rejected duplicate-root 'locked crate archive has ambiguous source cache roots'

printf 'cargo-deny locked archive cache hostile tests passed\n'
