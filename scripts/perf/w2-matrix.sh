#!/usr/bin/env bash
# W2 build/profile matrix driver.
#
# Builds selected redlinedb CLI profile/allocator variants, copies each
# binary to a stable target/perf path, optionally runs a perf lane, and
# records one JSONL manifest row per variant.
#
# Usage:
#   scripts/perf/w2-matrix.sh \
#     --suite none \
#     --profiles release,release-native \
#     --allocators mimalloc,jemalloc
#
# Profiles:
#   release            cargo --release, portable x86-64-v3 per .cargo/config.toml
#   release-native     release-native + REDLINE_BASE_RUSTFLAGS
#   release-pgo        scripts/perf/pgo.sh
#   release-pgo-bolt   scripts/perf/pgo.sh --for-bolt, then scripts/perf/bolt.sh
#
# Suites:
#   none, full

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

# shellcheck source=scripts/perf/lib-rustflags.sh
. "$(git rev-parse --show-toplevel)/scripts/perf/lib-rustflags.sh"

SUITE="${W2_SUITE:-none}"
PROFILES="${W2_PROFILES:-release,release-native}"
ALLOCATORS="${W2_ALLOCATORS:-mimalloc,jemalloc}"
RUN_ID="${W2_RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
DRY_RUN=0

usage() {
  sed -n '2,32p' "$0"
}

while [ $# -gt 0 ]; do
  case "$1" in
    --suite=*)
      SUITE="${1#*=}"
      shift
      ;;
    --suite)
      SUITE="${2:?--suite requires a value}"
      shift 2
      ;;
    --profiles=*)
      PROFILES="${1#*=}"
      shift
      ;;
    --profiles)
      PROFILES="${2:?--profiles requires a value}"
      shift 2
      ;;
    --allocators=*)
      ALLOCATORS="${1#*=}"
      shift
      ;;
    --allocators)
      ALLOCATORS="${2:?--allocators requires a value}"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'w2-matrix.sh: unknown argument: %s\n' "$1" >&2
      exit 2
      ;;
  esac
done

case "$SUITE" in
  none|full) ;;
  *)
    printf 'w2-matrix.sh: --suite must be one of {none,full}, got: %s\n' "$SUITE" >&2
    exit 2
    ;;
esac

IFS=',' read -r -a PROFILE_LIST <<< "$PROFILES"
IFS=',' read -r -a ALLOCATOR_LIST <<< "$ALLOCATORS"

if [ "${#PROFILE_LIST[@]}" -eq 0 ] || [ -z "${PROFILE_LIST[0]}" ]; then
  printf 'w2-matrix.sh: at least one profile is required\n' >&2
  exit 2
fi
if [ "${#ALLOCATOR_LIST[@]}" -eq 0 ] || [ -z "${ALLOCATOR_LIST[0]}" ]; then
  printf 'w2-matrix.sh: at least one allocator is required\n' >&2
  exit 2
fi

for profile in "${PROFILE_LIST[@]}"; do
  case "$profile" in
    release|release-native|release-pgo|release-pgo-bolt) ;;
    *)
      printf 'w2-matrix.sh: unknown profile: %s\n' "$profile" >&2
      exit 2
      ;;
  esac
done

for allocator in "${ALLOCATOR_LIST[@]}"; do
  case "$allocator" in
    mimalloc|jemalloc|snmalloc) ;;
    *)
      printf 'w2-matrix.sh: unknown allocator: %s\n' "$allocator" >&2
      printf '              expected one of {mimalloc,jemalloc,snmalloc}\n' >&2
      exit 2
      ;;
  esac
done

OUT_DIR="${PERF_ROOT:-target/perf}/w2-matrix/${RUN_ID}"
BIN_DIR="$OUT_DIR/bin"
MANIFEST="$OUT_DIR/manifest.jsonl"

run_cmd() {
  if [ "$DRY_RUN" = "1" ]; then
    printf '+'
    printf ' %q' "$@"
    printf '\n'
  else
    "$@"
  fi
}

set_allocator_args() {
  local allocator="$1"
  CARGO_ALLOCATOR_ARGS=(--no-default-features --features "alloc-${allocator}")
}

variant_label() {
  local profile="$1" allocator="$2"
  printf 'w2-%s-%s-%s' "$profile" "$allocator" "$RUN_ID"
}

copy_variant_bin() {
  local src="$1" dst="$2"
  if [ "$DRY_RUN" = "1" ]; then
    printf '+ mkdir -p %q\n' "$(dirname "$dst")"
    printf '+ cp %q %q\n' "$src" "$dst"
    return
  fi
  if [ ! -x "$src" ]; then
    printf 'w2-matrix.sh: expected binary missing: %s\n' "$src" >&2
    exit 1
  fi
  mkdir -p "$(dirname "$dst")"
  cp "$src" "$dst"
  chmod +x "$dst"
}

write_manifest_entry() {
  local profile="$1" allocator="$2" label="$3" bin="$4" perf_jsonl="$5"
  if [ "$DRY_RUN" = "1" ]; then
    return
  fi
  local -a perf_jsonl_arg=()
  if [ -n "$perf_jsonl" ]; then
    perf_jsonl_arg=(--perf-jsonl "$perf_jsonl")
  fi
  cargo run --quiet --locked -p redlinedb-bench --bin perf_evidence -- \
    append-w2-manifest \
    --output "$MANIFEST" \
    --profile "$profile" \
    --allocator "$allocator" \
    --label "$label" \
    --binary "$bin" \
    --suite "$SUITE" \
    "${perf_jsonl_arg[@]}" \
    --base-rustflags="$REDLINE_BASE_RUSTFLAGS"
}

build_variant() {
  local profile="$1" allocator="$2" label="$3" out_bin="$4"
  set_allocator_args "$allocator"
  local src_bin

  case "$profile" in
    release)
      run_cmd cargo build --release -p redlinedb-cli --bin redlinedb --locked "${CARGO_ALLOCATOR_ARGS[@]}"
      src_bin="target/release/redlinedb"
      ;;
    release-native)
      run_cmd env RUSTFLAGS="$REDLINE_BASE_RUSTFLAGS" \
        cargo build --profile release-native -p redlinedb-cli --bin redlinedb --locked "${CARGO_ALLOCATOR_ARGS[@]}"
      src_bin="target/release-native/redlinedb"
      ;;
    release-pgo)
      run_cmd env REDLINE_CARGO_FEATURE_ARGS="${CARGO_ALLOCATOR_ARGS[*]}" \
        bash scripts/perf/pgo.sh
      src_bin="target/release-pgo/redlinedb"
      ;;
    release-pgo-bolt)
      run_cmd env REDLINE_CARGO_FEATURE_ARGS="${CARGO_ALLOCATOR_ARGS[*]}" \
        bash scripts/perf/pgo.sh --for-bolt
      run_cmd bash scripts/perf/bolt.sh
      src_bin="target/release-pgo/redlinedb.bolt"
      ;;
    *)
      printf 'w2-matrix.sh: unreachable profile: %s\n' "$profile" >&2
      exit 2
      ;;
  esac

  copy_variant_bin "$src_bin" "$out_bin"
  printf '==> built %s (%s/%s) -> %s\n' "$label" "$profile" "$allocator" "$out_bin"
}

run_perf_lane() {
  local bin="$1" label="$2"
  if [ "$SUITE" = "none" ]; then
    return
  fi
  run_cmd bash "scripts/perf/${SUITE}.sh" "$bin" "$label"
}

if [ "$DRY_RUN" = "1" ]; then
  printf '==> w2-matrix.sh DRY-RUN\n'
else
  mkdir -p "$BIN_DIR"
  : > "$MANIFEST"
fi

printf 'run id:      %s\n' "$RUN_ID"
printf 'suite:       %s\n' "$SUITE"
printf 'profiles:    %s\n' "$PROFILES"
printf 'allocators:  %s\n' "$ALLOCATORS"
printf 'pgo corpus:  full (external redline-testing)\n'
printf 'output dir:  %s\n' "$OUT_DIR"

for profile in "${PROFILE_LIST[@]}"; do
  for allocator in "${ALLOCATOR_LIST[@]}"; do
    label="$(variant_label "$profile" "$allocator")"
    out_bin="$BIN_DIR/redlinedb-${profile}-${allocator}"
    perf_jsonl=""
    if [ "$SUITE" != "none" ]; then
      perf_jsonl="${PERF_ROOT:-target/perf}/${label}.jsonl"
    fi
    printf '\n==> W2 variant: %s\n' "$label"
    build_variant "$profile" "$allocator" "$label" "$out_bin"
    run_perf_lane "$out_bin" "$label"
    write_manifest_entry "$profile" "$allocator" "$label" "$out_bin" "$perf_jsonl"
  done
done

if [ "$DRY_RUN" = "1" ]; then
  printf '\nDry-run complete; no files written.\n'
else
  printf '\nW2 matrix complete.\n'
  printf 'manifest: %s\n' "$MANIFEST"
fi
