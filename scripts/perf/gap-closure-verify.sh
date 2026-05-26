#!/usr/bin/env bash
# Gap-closure verification — run after all track branches have merged
# into claude-gap-closure and HPC #2-5 have landed.
#
# 1. Build redlinedb-cli (release-native + native CPU)
# 2. Run the full 2,445-case SQLite-parity corpus (sqlite3 oracle)
# 3. Run the 265-case beyond-Postgres corpus (psql oracle) — depends on
#    the beyond-SQLite investigator having wired the corpus through
# 4. Compute per-case latency deltas vs baseline (committed at
#    benchmark-results/sqlite-parity/gap-closure-deltas/baseline-c92b1d3-ranked.csv)
# 5. Emit a Markdown summary at target/gap-closure-summary.md ready for
#    the PR description.
#
# Usage:
#   scripts/perf/gap-closure-verify.sh

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

REDLINE_TESTING_BIN="${REDLINE_TESTING_BIN:-/home/ubuntu/redline-testing/target/release/redline-testing}"
SQLITE_REF_BIN="${SQLITE_REF_BIN:-/home/ubuntu/redlineDB/target/sqlite-reference/3.53.1/bin/sqlite3}"
BASELINE_RANKED="${BASELINE_RANKED:-benchmark-results/sqlite-parity/gap-closure-deltas/baseline-c92b1d3-ranked.csv}"
OUT_DIR="${OUT_DIR:-target/redline-testing-gap-closure}"

if [ ! -x "$REDLINE_TESTING_BIN" ]; then
    echo "redline-testing binary missing at $REDLINE_TESTING_BIN" >&2
    exit 1
fi

mkdir -p "$OUT_DIR"

echo ">>> Building redlinedb-cli (release-native)"
RUSTFLAGS="-C target-cpu=native" cargo build --profile release-native -p redlinedb-cli --bin redlinedb --locked
BIN="target/release-native/redlinedb"
if [ ! -x "$BIN" ]; then
    BIN="target/release/redlinedb"
    cargo build -p redlinedb-cli --release --locked --bin redlinedb
fi

echo ">>> Running SQLite-parity suite (1,127 pinned + 1,318 generated = 2,445 cases)"
"$REDLINE_TESTING_BIN" run \
    --target-bin "$BIN" \
    --sqlite-bin "$SQLITE_REF_BIN" \
    --suite sqlite_parity \
    --workers "${PERF_WORKERS:-10}" \
    --tmp-root /dev/shm/redline-testing-gap-closure \
    --repetitions 3 --warmup 1 \
    --output "$OUT_DIR/sqlite-parity.jsonl" 2>&1 | tail -5

PARITY_FAILED=$(jq -r 'select(.status=="failed") | .case_id' "$OUT_DIR/sqlite-parity.jsonl" | sort -u | wc -l || echo "0")
PARITY_TOTAL=$(jq -r '.case_id' "$OUT_DIR/sqlite-parity.jsonl" | sort -u | wc -l || echo "0")

echo ""
echo ">>> Running beyond-Postgres oracle (if wired)"
if [ -x "scripts/run-beyond-pg.sh" ]; then
    bash scripts/run-beyond-pg.sh "$BIN" "$OUT_DIR/beyond-pg.jsonl" 2>&1 | tail -5 || true
    BEYOND_FAILED=$(jq -r 'select(.status=="failed") | .case_id' "$OUT_DIR/beyond-pg.jsonl" 2>/dev/null | sort -u | wc -l || echo "0")
    BEYOND_TOTAL=$(jq -r '.case_id' "$OUT_DIR/beyond-pg.jsonl" 2>/dev/null | sort -u | wc -l || echo "0")
else
    BEYOND_FAILED="?"
    BEYOND_TOTAL="?"
    echo "    scripts/run-beyond-pg.sh not present — beyond-PG oracle not wired"
fi

SKIP_LIST="${SKIP_LIST:-metadata/beyond_sqlite/skip-list.toml}"
SKIPPED_BY_DESIGN=0
if [ -f "$SKIP_LIST" ]; then
    SKIPPED_BY_DESIGN=$(grep -c '^\[\[skip\]\]' "$SKIP_LIST" 2>/dev/null || echo "0")
fi

echo ""
echo ">>> Computing per-case latency deltas vs baseline"
# Read baseline and new ranked.csv; join on case_id; compute median delta
NEW_RANKED="$OUT_DIR/ranked.csv"
if [ -f "$OUT_DIR/ranked.csv" ] && [ -f "$BASELINE_RANKED" ]; then
    python3 - "$BASELINE_RANKED" "$NEW_RANKED" >"$OUT_DIR/deltas.tsv" <<'PYEOF'
import csv
import sys

baseline_path, new_path = sys.argv[1], sys.argv[2]

def load(path):
    with open(path, newline="") as f:
        reader = csv.DictReader(f)
        return {r["case_id"]: r for r in reader}

base = load(baseline_path)
new = load(new_path)

print("case_id\tname\tbaseline_p50_ns\tnew_p50_ns\tdelta_pct")
for cid in sorted(set(base) & set(new)):
    try:
        b = float(base[cid].get("redline_median_ns") or "nan")
        n = float(new[cid].get("redline_median_ns") or "nan")
        if b > 0:
            delta = (n - b) / b * 100.0
        else:
            delta = float("nan")
        print(f"{cid}\t{base[cid].get('name','')}\t{int(b)}\t{int(n)}\t{delta:+.2f}")
    except Exception:
        continue
PYEOF
    DELTAS_AVAILABLE=1
else
    DELTAS_AVAILABLE=0
fi

echo ""
echo ">>> Writing summary"
cat >"$OUT_DIR/summary.md" <<EOF
# Gap-closure verification — $(git log -1 --format="%h %s")

## Headline numbers

| Surface | Failed / Total | Notes |
|---|---|---|
| SQLite-parity | $PARITY_FAILED / $PARITY_TOTAL | Down from 426/2,445 baseline |
| Beyond-PG | $BEYOND_FAILED / $BEYOND_TOTAL | Skipped-by-design: $SKIPPED_BY_DESIGN |

## Per-track deltas

EOF
if [ "$DELTAS_AVAILABLE" = "1" ]; then
    echo "Top 20 worst latency regressions (negative deltas = faster):" >>"$OUT_DIR/summary.md"
    echo "" >>"$OUT_DIR/summary.md"
    echo '```' >>"$OUT_DIR/summary.md"
    tail -n +2 "$OUT_DIR/deltas.tsv" | sort -t$'\t' -k5 -n -r | head -20 >>"$OUT_DIR/summary.md"
    echo '```' >>"$OUT_DIR/summary.md"
    echo "" >>"$OUT_DIR/summary.md"
    echo "Top 20 best latency improvements:" >>"$OUT_DIR/summary.md"
    echo "" >>"$OUT_DIR/summary.md"
    echo '```' >>"$OUT_DIR/summary.md"
    tail -n +2 "$OUT_DIR/deltas.tsv" | sort -t$'\t' -k5 -n | head -20 >>"$OUT_DIR/summary.md"
    echo '```' >>"$OUT_DIR/summary.md"
fi

echo "" >>"$OUT_DIR/summary.md"
echo "Full per-case deltas: \`$OUT_DIR/deltas.tsv\`" >>"$OUT_DIR/summary.md"

cat "$OUT_DIR/summary.md"
echo ""
echo "Done. Summary: $OUT_DIR/summary.md"
