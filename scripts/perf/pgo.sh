#!/usr/bin/env bash
# PGO (profile-guided optimization) build for the redlinedb CLI binary.
#
# Two-pass workflow:
#   1. Build an instrumented binary with `-Cprofile-generate`.
#   2. Run the redline-testing parity workload against the instrumented
#      binary to capture profile data.
#   3. Build the final binary with `-Cprofile-use` reading the captured
#      profile.
#
# Run this AFTER the other HPC steps (release-native + ahash + mimalloc +
# interning) have landed, so the profile captures the actual hot paths
# under the optimized configuration.
#
# Usage:
#   scripts/perf/pgo.sh [--for-bolt] [--dry-run]
#
# Training always uses the complete official corpus through the verified
# external `redline-testing` runner. Redline core does not own a subset
# replay producer.
#
# Flags:
#   --for-bolt   Add `-Wl,--emit-relocs` to the final link so the binary
#                can be post-processed by scripts/perf/bolt.sh.
#   --dry-run    Print the workload command(s) and exit without building
#                or running anything heavy.
#
# Outputs:
#   target/release-pgo/redlinedb  -- the final optimized binary
#   /tmp/redlinedb-pgo-data.*     -- raw profile data (regenerate per build)

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

# shellcheck source=scripts/perf/lib-rustflags.sh
. "$(git rev-parse --show-toplevel)/scripts/perf/lib-rustflags.sh"

FOR_BOLT=0
DRY_RUN=0

while [ $# -gt 0 ]; do
    case "$1" in
        --for-bolt)
            FOR_BOLT=1
            shift
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        -h|--help)
            sed -n '2,30p' "$0"
            exit 0
            ;;
        *)
            echo "pgo.sh: unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

PGO_DATA_DIR="${PGO_DATA_DIR:-}"
PGO_PROFILE_DIR="${PGO_PROFILE_DIR:-}"
REDLINE_CARGO_FEATURE_ARGS_STR="${REDLINE_CARGO_FEATURE_ARGS:-}"
REDLINE_CARGO_FEATURE_ARGS=()
if [ -n "$REDLINE_CARGO_FEATURE_ARGS_STR" ]; then
    # shellcheck disable=SC2206
    REDLINE_CARGO_FEATURE_ARGS=($REDLINE_CARGO_FEATURE_ARGS_STR)
fi
REDLINE_SPLIT_ROOT="${REDLINE_SPLIT_ROOT:-$(cd ".." && pwd)}"
REDLINE_TESTING_BIN="${REDLINE_TESTING_BIN:-${REDLINE_SPLIT_ROOT}/redline-testing/target/release/redline-testing}"
SQLITE_REF_BIN="${SQLITE_REF_BIN:-$(bash scripts/sqlite/build-reference.sh 2>/dev/null || echo "${REDLINE_SPLIT_ROOT}/sqlite-reference/bin/sqlite3")}" 

if [ "$DRY_RUN" = "0" ]; then
    if [ -z "$PGO_DATA_DIR" ]; then
        PGO_DATA_DIR="$(mktemp -d "${TMPDIR:-/tmp}/redlinedb-pgo-data.XXXXXX")"
    fi
    if [ -z "$PGO_PROFILE_DIR" ]; then
        PGO_PROFILE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/redlinedb-pgo-profile.XXXXXX")"
    fi
else
    PGO_DATA_DIR="${PGO_DATA_DIR:-/tmp/redlinedb-pgo-data}"
    PGO_PROFILE_DIR="${PGO_PROFILE_DIR:-/tmp/redlinedb-pgo-profile}"
fi

if [ ! -x "$REDLINE_TESTING_BIN" ]; then
    echo "redline-testing binary not found at $REDLINE_TESTING_BIN" >&2
    echo "Build it in the sibling redline-testing checkout: cargo build --release --locked --bin redline-testing" >&2
    [ "$DRY_RUN" = "1" ] || exit 1
fi
if [ ! -x "$SQLITE_REF_BIN" ]; then
    echo "sqlite3 reference not found at $SQLITE_REF_BIN" >&2
    [ "$DRY_RUN" = "1" ] || exit 1
fi

# Locate llvm-profdata (matching the rustc toolchain's LLVM)
LLVM_PROFDATA="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's|host: ||p')/bin/llvm-profdata"
if [ ! -x "$LLVM_PROFDATA" ]; then
    # rustup-shipped fallback
    LLVM_PROFDATA="$(rustup which llvm-profdata 2>/dev/null || command -v llvm-profdata || true)"
fi
if [ -z "$LLVM_PROFDATA" ] || [ ! -x "$LLVM_PROFDATA" ]; then
    echo "llvm-profdata not found. Install with: rustup component add llvm-tools-preview" >&2
    [ "$DRY_RUN" = "1" ] || exit 1
fi

INSTR_BIN="target/release-pgo/redlinedb"

# Final-link RUSTFLAGS — append --emit-relocs when --for-bolt so the
# bolt.sh post-link step has the relocations it needs to rewrite.
FINAL_LINK_EXTRA=""
if [ "$FOR_BOLT" = "1" ]; then
    FINAL_LINK_EXTRA=" -C link-arg=-Wl,--emit-relocs"
fi

# Print the full external workload under --dry-run so users can sanity-check
# the command before committing to the multi-hour pipeline.
print_workload_cmd() {
    cat <<EOF
"$REDLINE_TESTING_BIN" run \\
    --target-bin "$INSTR_BIN" \\
    --sqlite-bin "$SQLITE_REF_BIN" \\
    --suite sqlite_parity \\
    --workers "\${PERF_WORKERS:-10}" \\
    --tmp-root /dev/shm/redline-testing-pgo \\
    --repetitions 1 --warmup 0 \\
    --output target/redline-testing-pgo/training.jsonl
EOF
}

if [ "$DRY_RUN" = "1" ]; then
    echo "==> pgo.sh DRY-RUN"
    echo "training corpus: full official corpus via redline-testing run"
    echo "for-bolt:        $FOR_BOLT"
    echo "final RUSTFLAGS: ${REDLINE_BASE_RUSTFLAGS}${FINAL_LINK_EXTRA} -Cprofile-use=$PGO_PROFILE_DIR/merged.profdata -Cllvm-args=-pgo-warn-missing-function"
    echo
    echo "would run workload:"
    print_workload_cmd
    exit 0
fi

echo ">>> [1/3] Cleaning previous PGO state"
rm -rf "$PGO_DATA_DIR" "$PGO_PROFILE_DIR"
mkdir -p "$PGO_DATA_DIR" "$PGO_PROFILE_DIR"

echo ">>> [2/3] Building instrumented binary"
RUSTFLAGS="${REDLINE_BASE_RUSTFLAGS} -Cprofile-generate=$PGO_DATA_DIR" \
    cargo build --profile release-pgo -p redlinedb-cli --bin redlinedb --locked "${REDLINE_CARGO_FEATURE_ARGS[@]}"

if [ ! -x "$INSTR_BIN" ]; then
    echo "Instrumented binary not found at $INSTR_BIN" >&2
    exit 1
fi

echo ">>> [3a/3] Running full official training corpus to gather profile data"
mkdir -p target/redline-testing-pgo
# We allow a non-zero exit here (|| true) because the instrumented binary
# emits extra stderr (durability notice, LLVM profile warnings) that the
# parity harness counts as failures. The .profraw files are written by the
# LLVM runtime regardless, so the profile is still valid.
REDLINEDB_DEFAULT_DURABILITY=normal \
REDLINEDB_QUIET_DURABILITY=1 \
"$REDLINE_TESTING_BIN" run \
    --target-bin "$INSTR_BIN" \
    --sqlite-bin "$SQLITE_REF_BIN" \
    --suite sqlite_parity \
    --workers "${PERF_WORKERS:-10}" \
    --tmp-root /dev/shm/redline-testing-pgo \
    --repetitions 1 --warmup 0 \
    --output target/redline-testing-pgo/training.jsonl \
|| true

echo ">>> [3b/3] Merging .profraw files"
"$LLVM_PROFDATA" merge -output="$PGO_PROFILE_DIR/merged.profdata" "$PGO_DATA_DIR"/*.profraw

echo ">>> [3c/3] Building final PGO-optimized binary"
RUSTFLAGS="${REDLINE_BASE_RUSTFLAGS}${FINAL_LINK_EXTRA} -Cprofile-use=$PGO_PROFILE_DIR/merged.profdata -Cllvm-args=-pgo-warn-missing-function" \
    cargo build --profile release-pgo -p redlinedb-cli --bin redlinedb --locked "${REDLINE_CARGO_FEATURE_ARGS[@]}"

echo ""
echo "Done."
echo "Final PGO-optimized binary: target/release-pgo/redlinedb"
echo "Profile data: $PGO_PROFILE_DIR/merged.profdata"
echo "Training corpus: full official corpus"
if [ "$FOR_BOLT" = "1" ]; then
    echo "Linked with --emit-relocs for BOLT post-processing (scripts/perf/bolt.sh)."
fi
