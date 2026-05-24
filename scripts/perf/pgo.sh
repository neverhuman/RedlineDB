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
#   scripts/perf/pgo.sh
#
# Outputs:
#   target/release-pgo/redlinedb  -- the final optimized binary
#   /tmp/redlinedb-pgo-data/      -- raw profile data (regenerate per build)

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

PGO_DATA_DIR="${PGO_DATA_DIR:-/tmp/redlinedb-pgo-data}"
PGO_PROFILE_DIR="${PGO_PROFILE_DIR:-/tmp/redlinedb-pgo-profile}"
REDLINE_TESTING_BIN="${REDLINE_TESTING_BIN:-/home/ubuntu/redline-testing/target/release/redline-testing}"
SQLITE_REF_BIN="${SQLITE_REF_BIN:-$(bash scripts/sqlite/build-reference.sh 2>/dev/null || echo "/home/ubuntu/redlineDB/target/sqlite-reference/3.53.1/bin/sqlite3")}"

if [ ! -x "$REDLINE_TESTING_BIN" ]; then
    echo "redline-testing binary not found at $REDLINE_TESTING_BIN" >&2
    echo "Build it: cd /home/ubuntu/redline-testing && cargo build --release --locked --bin redline-testing" >&2
    exit 1
fi
if [ ! -x "$SQLITE_REF_BIN" ]; then
    echo "sqlite3 reference not found at $SQLITE_REF_BIN" >&2
    exit 1
fi

# Locate llvm-profdata (matching the rustc toolchain's LLVM)
LLVM_PROFDATA="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's|host: ||p')/bin/llvm-profdata"
if [ ! -x "$LLVM_PROFDATA" ]; then
    # rustup-shipped fallback
    LLVM_PROFDATA="$(rustup which llvm-profdata 2>/dev/null || command -v llvm-profdata || true)"
fi
if [ -z "$LLVM_PROFDATA" ] || [ ! -x "$LLVM_PROFDATA" ]; then
    echo "llvm-profdata not found. Install with: rustup component add llvm-tools-preview" >&2
    exit 1
fi

echo ">>> [1/3] Cleaning previous PGO state"
rm -rf "$PGO_DATA_DIR" "$PGO_PROFILE_DIR"
mkdir -p "$PGO_DATA_DIR" "$PGO_PROFILE_DIR"

echo ">>> [2/3] Building instrumented binary"
RUSTFLAGS="-C target-cpu=native -Cprofile-generate=$PGO_DATA_DIR" \
    cargo build --profile release-pgo -p redlinedb-cli --bin redlinedb --locked

INSTR_BIN="target/release-pgo/redlinedb"
if [ ! -x "$INSTR_BIN" ]; then
    echo "Instrumented binary not found at $INSTR_BIN" >&2
    exit 1
fi

echo ">>> [3a/3] Running parity workload to gather profile data"
mkdir -p target/redline-testing-pgo
"$REDLINE_TESTING_BIN" run \
    --target-bin "$INSTR_BIN" \
    --sqlite-bin "$SQLITE_REF_BIN" \
    --suite sqlite_parity \
    --workers auto \
    --tmp-root /dev/shm/redline-testing-pgo \
    --repetitions 1 --warmup 0 \
    --output target/redline-testing-pgo/training.jsonl

echo ">>> [3b/3] Merging .profraw files"
"$LLVM_PROFDATA" merge -output="$PGO_PROFILE_DIR/merged.profdata" "$PGO_DATA_DIR"/*.profraw

echo ">>> [3c/3] Building final PGO-optimized binary"
RUSTFLAGS="-C target-cpu=native -Cprofile-use=$PGO_PROFILE_DIR/merged.profdata -Cllvm-args=-pgo-warn-missing-function" \
    cargo build --profile release-pgo -p redlinedb-cli --bin redlinedb --locked

echo ""
echo "Done."
echo "Final PGO-optimized binary: target/release-pgo/redlinedb"
echo "Profile data: $PGO_PROFILE_DIR/merged.profdata"
