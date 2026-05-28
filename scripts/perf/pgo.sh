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
#   scripts/perf/pgo.sh [--training-subset {quick,medium,full}] [--for-bolt] [--dry-run]
#
# Training subsets (default: medium):
#   quick   ~40 curated cases via scripts/perf/run_subset.py (fast smoke).
#   medium  ~300 curated cases via scripts/perf/run_subset.py (default;
#           stratified by category x priority x profile).
#   full    1127-case official corpus via `redline-testing run`
#           (mirrors CI; original behavior).
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

TRAINING_SUBSET="medium"
FOR_BOLT=0
DRY_RUN=0

while [ $# -gt 0 ]; do
    case "$1" in
        --training-subset=*)
            TRAINING_SUBSET="${1#*=}"
            shift
            ;;
        --training-subset)
            TRAINING_SUBSET="${2:?--training-subset requires a value}"
            shift 2
            ;;
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

case "$TRAINING_SUBSET" in
    quick|medium|full) ;;
    *)
        echo "pgo.sh: --training-subset must be one of {quick,medium,full}, got: $TRAINING_SUBSET" >&2
        exit 2
        ;;
esac

PGO_DATA_DIR="${PGO_DATA_DIR:-}"
PGO_PROFILE_DIR="${PGO_PROFILE_DIR:-}"
REDLINE_CARGO_FEATURE_ARGS_STR="${REDLINE_CARGO_FEATURE_ARGS:-}"
REDLINE_CARGO_FEATURE_ARGS=()
if [ -n "$REDLINE_CARGO_FEATURE_ARGS_STR" ]; then
    # shellcheck disable=SC2206
    REDLINE_CARGO_FEATURE_ARGS=($REDLINE_CARGO_FEATURE_ARGS_STR)
fi
REDLINE_TESTING_BIN="${REDLINE_TESTING_BIN:-/home/ubuntu/redline-testing/target/release/redline-testing}"
SQLITE_REF_BIN="${SQLITE_REF_BIN:-$(bash scripts/sqlite/build-reference.sh 2>/dev/null || echo "/home/ubuntu/redlineDB/target/sqlite-reference/3.53.1/bin/sqlite3")}"
PERF_CASES_DIR="${PERF_CASES_DIR:-bench/perf/cases}"
PERF_ROOT="${PERF_ROOT:-target/perf}"

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

# Resolve subset-specific case list. `build_case_lists.py` is the
# canonical generator (see scripts/perf/build-case-lists.sh); the
# committed lists in $PERF_CASES_DIR are its checked-in output and
# match the schema run_subset.py expects (one 5-digit id per line,
# `#` comments allowed).
CASE_LIST=""
case "$TRAINING_SUBSET" in
    quick)  CASE_LIST="$PERF_CASES_DIR/quick-set.txt" ;;
    medium) CASE_LIST="$PERF_CASES_DIR/medium-set.txt" ;;
    full)   CASE_LIST="" ;;
esac

if [ -n "$CASE_LIST" ] && [ ! -f "$CASE_LIST" ]; then
    echo "pgo.sh: case-list missing at $CASE_LIST" >&2
    echo "         run scripts/perf/build-case-lists.sh to regenerate, or" >&2
    echo "         re-run with --training-subset=full to skip case-list filtering." >&2
    exit 2
fi

if [ ! -x "$REDLINE_TESTING_BIN" ]; then
    echo "redline-testing binary not found at $REDLINE_TESTING_BIN" >&2
    echo "Build it: cd /home/ubuntu/redline-testing && cargo build --release --locked --bin redline-testing" >&2
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
SNAPSHOT="$PERF_ROOT/corpus-snapshot.json"

# Final-link RUSTFLAGS — append --emit-relocs when --for-bolt so the
# bolt.sh post-link step has the relocations it needs to rewrite.
FINAL_LINK_EXTRA=""
if [ "$FOR_BOLT" = "1" ]; then
    FINAL_LINK_EXTRA=" -C link-arg=-Wl,--emit-relocs"
fi

# Compose the workload command for the chosen subset. We print it under
# --dry-run so users can sanity-check before committing to the full
# multi-hour pipeline.
print_workload_cmd() {
    if [ "$TRAINING_SUBSET" = "full" ]; then
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
    else
        cat <<EOF
python3 scripts/perf/run_subset.py \\
    --case-list   "$CASE_LIST" \\
    --target-bin  "$INSTR_BIN" \\
    --sqlite-bin  "$SQLITE_REF_BIN" \\
    --output      target/redline-testing-pgo/training.jsonl \\
    --repetitions 1 --warmup 0 \\
    --snapshot    "$SNAPSHOT" \\
    --tmp-root    /dev/shm/redline-testing-pgo \\
    --workers     "\${PERF_WORKERS:-1}"
EOF
    fi
}

if [ "$DRY_RUN" = "1" ]; then
    echo "==> pgo.sh DRY-RUN"
    echo "training subset: $TRAINING_SUBSET"
    if [ -n "$CASE_LIST" ]; then
        echo "case list:       $CASE_LIST"
    else
        echo "case list:       <full corpus via redline-testing run>"
    fi
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

echo ">>> [3a/3] Running training workload (subset=$TRAINING_SUBSET) to gather profile data"
mkdir -p target/redline-testing-pgo
if [ "$TRAINING_SUBSET" = "full" ]; then
    # We allow a non-zero exit here (|| true) because the instrumented binary
    # emits extra stderr (durability notice, LLVM profile warnings) that the
    # parity harness counts as failures.  The .profraw files are written by
    # the LLVM runtime regardless, so the profile is still valid.
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
else
    # Subset path: replay only the curated case-list via run_subset.py.
    # The instrumented binary still writes .profraw to $PGO_DATA_DIR on
    # every invocation (LLVM runtime, not the harness), so this captures
    # a valid profile from the targeted hot cases.
    if [ ! -f "$SNAPSHOT" ]; then
        echo ">>> snapshot missing at $SNAPSHOT — regenerating via redline-testing list"
        mkdir -p "$(dirname "$SNAPSHOT")"
        "$REDLINE_TESTING_BIN" list --suite sqlite_parity --format json > "$SNAPSHOT"
    fi
    mkdir -p /dev/shm/redline-testing-pgo
    REDLINEDB_DEFAULT_DURABILITY=normal \
    python3 scripts/perf/run_subset.py \
        --case-list   "$CASE_LIST" \
        --target-bin  "$INSTR_BIN" \
        --sqlite-bin  "$SQLITE_REF_BIN" \
        --output      target/redline-testing-pgo/training.jsonl \
        --repetitions 1 --warmup 0 \
        --snapshot    "$SNAPSHOT" \
        --tmp-root    /dev/shm/redline-testing-pgo \
        --workers     "${PERF_WORKERS:-1}"
fi

echo ">>> [3b/3] Merging .profraw files"
"$LLVM_PROFDATA" merge -output="$PGO_PROFILE_DIR/merged.profdata" "$PGO_DATA_DIR"/*.profraw

echo ">>> [3c/3] Building final PGO-optimized binary"
RUSTFLAGS="${REDLINE_BASE_RUSTFLAGS}${FINAL_LINK_EXTRA} -Cprofile-use=$PGO_PROFILE_DIR/merged.profdata -Cllvm-args=-pgo-warn-missing-function" \
    cargo build --profile release-pgo -p redlinedb-cli --bin redlinedb --locked "${REDLINE_CARGO_FEATURE_ARGS[@]}"

echo ""
echo "Done."
echo "Final PGO-optimized binary: target/release-pgo/redlinedb"
echo "Profile data: $PGO_PROFILE_DIR/merged.profdata"
echo "Training subset: $TRAINING_SUBSET"
if [ "$FOR_BOLT" = "1" ]; then
    echo "Linked with --emit-relocs for BOLT post-processing (scripts/perf/bolt.sh)."
fi
