#!/usr/bin/env bash
# BOLT (Binary Optimization and Layout Tool) post-link pass for the
# PGO-built redlinedb binary.
#
# Pipeline:
#   1. perf record -e cycles:u -j any,u,k         (sample LBR profile)
#   2. perf2bolt -> .fdata                        (convert to BOLT input)
#   3. llvm-bolt with ext-tsp + hfsort+ + split   (rewrite the binary)
#
# Prerequisites:
#   * Linux x86_64. llvm-bolt's branch-relayout for AArch64 is still
#     incomplete upstream as of LLVM 19; we refuse to run there.
#   * llvm-bolt v18 or newer.
#   * `perf` with LBR support (cycles:u -j any,u,k).
#   * A PGO build done with --emit-relocs in the link step. Run:
#         scripts/perf/pgo.sh --for-bolt
#     first; otherwise BOLT cannot rewrite the binary.
#
# Usage:
#   scripts/perf/bolt.sh
#
# Outputs:
#   target/redlinedb.perf.data        -- raw perf samples (regenerated)
#   target/redlinedb.bolt.fdata       -- BOLT-formatted profile
#   target/release-pgo/redlinedb.bolt -- final BOLT-optimized binary

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

# shellcheck source=scripts/perf/lib-rustflags.sh
. "$(git rev-parse --show-toplevel)/scripts/perf/lib-rustflags.sh"

if [ "$(uname -m)" != "x86_64" ]; then
    echo "bolt.sh: requires x86_64 (got $(uname -m))." >&2
    echo "         llvm-bolt AArch64 branch-relayout is not production-ready as of LLVM 19." >&2
    exit 2
fi

if ! command -v llvm-bolt >/dev/null 2>&1; then
    echo "bolt.sh: llvm-bolt not on PATH." >&2
    echo "         Install via: apt-get install llvm-18 (or your distro's equivalent)" >&2
    echo "         and ensure llvm-bolt / perf2bolt are reachable." >&2
    exit 2
fi
BOLT_VERSION_RAW="$(llvm-bolt --version 2>&1 | head -1)"
BOLT_MAJOR="$(printf '%s\n' "$BOLT_VERSION_RAW" | sed -n 's/.*LLVM version \([0-9]\+\).*/\1/p')"
if [ -z "$BOLT_MAJOR" ] || [ "$BOLT_MAJOR" -lt 18 ]; then
    echo "bolt.sh: llvm-bolt v18+ required (got: $BOLT_VERSION_RAW)." >&2
    exit 2
fi

if ! command -v perf2bolt >/dev/null 2>&1; then
    echo "bolt.sh: perf2bolt not on PATH (ships with llvm-bolt)." >&2
    exit 2
fi

if ! command -v perf >/dev/null 2>&1; then
    echo "bolt.sh: linux 'perf' not on PATH." >&2
    echo "         Install via: apt-get install linux-tools-common linux-tools-\$(uname -r)" >&2
    exit 2
fi
PERF_VERSION_RAW="$(perf --version 2>&1 | head -1)"
echo "==> tooling: $BOLT_VERSION_RAW / $PERF_VERSION_RAW"

INPUT_BIN="target/release-pgo/redlinedb"
if [ ! -x "$INPUT_BIN" ]; then
    echo "bolt.sh: input binary missing at $INPUT_BIN" >&2
    echo "         run scripts/perf/pgo.sh --for-bolt first" >&2
    exit 2
fi

# Sanity-check that the binary was linked with --emit-relocs. BOLT will
# refuse to rewrite an executable without a .rela.text-style section
# (it bails out with "BOLT-ERROR: relocations against code are missing").
if ! readelf -S "$INPUT_BIN" 2>/dev/null | grep -q '\.rela\.text\|\.rel\.text'; then
    echo "bolt.sh: $INPUT_BIN appears to lack code relocations (.rela.text)." >&2
    echo "         Rebuild with: scripts/perf/pgo.sh --for-bolt" >&2
    exit 2
fi

REDLINE_SPLIT_ROOT="${REDLINE_SPLIT_ROOT:-$(cd ".." && pwd)}"
REDLINE_TESTING_BIN="${REDLINE_TESTING_BIN:-${REDLINE_SPLIT_ROOT}/redline-testing/target/release/redline-testing}"
SQLITE_REF_BIN="${SQLITE_REF_BIN:-$(bash scripts/sqlite/build-reference.sh 2>/dev/null || echo "${REDLINE_SPLIT_ROOT}/sqlite-reference/bin/sqlite3")}" 
if [ ! -x "$REDLINE_TESTING_BIN" ]; then
    echo "bolt.sh: redline-testing not at $REDLINE_TESTING_BIN" >&2
    exit 2
fi
if [ ! -x "$SQLITE_REF_BIN" ]; then
    echo "bolt.sh: sqlite3 reference not at $SQLITE_REF_BIN" >&2
    exit 2
fi

PERF_DATA="target/redlinedb.perf.data"
BOLT_FDATA="target/redlinedb.bolt.fdata"
BOLT_OUT="target/release-pgo/redlinedb.bolt"

mkdir -p target /dev/shm/redline-testing-bolt 2>/dev/null || mkdir -p target

echo ">>> [1/3] perf record (cycles:u, LBR any,u,k) -> $PERF_DATA"
rm -f "$PERF_DATA"
perf record \
    -e cycles:u \
    -j any,u,k \
    -o "$PERF_DATA" \
    -- "$REDLINE_TESTING_BIN" run \
        --target-bin "$INPUT_BIN" \
        --sqlite-bin "$SQLITE_REF_BIN" \
        --suite sqlite_parity \
        --workers "${PERF_WORKERS:-10}" \
        --tmp-root /dev/shm/redline-testing-bolt \
        --repetitions 1 \
        --warmup 0 \
        --output target/redline-testing-bolt/training.jsonl

echo ">>> [2/3] perf2bolt -> $BOLT_FDATA"
perf2bolt \
    -p "$PERF_DATA" \
    -o "$BOLT_FDATA" \
    "$INPUT_BIN"

echo ">>> [3/3] llvm-bolt rewrite -> $BOLT_OUT"
llvm-bolt "$INPUT_BIN" \
    -o "$BOLT_OUT" \
    -data="$BOLT_FDATA" \
    -reorder-blocks=ext-tsp \
    -reorder-functions=hfsort+ \
    -split-functions \
    -split-all-cold \
    -split-eh \
    -dyno-stats

echo ""
echo "Done."
echo "BOLT-optimized binary: $BOLT_OUT"
echo "  size: $(stat -c %s "$BOLT_OUT" 2>/dev/null || stat -f %z "$BOLT_OUT") bytes"
echo "Profile: $BOLT_FDATA"
