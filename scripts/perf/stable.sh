#!/usr/bin/env bash
# W9-S: stable.sh — run scripts/perf/full.sh N times and report
# median-of-medians + per-run spread so sub-variance improvements
# (the A24/A26 class of fix that saves syscalls but shows < 5% on a
# single run) become visible.
#
# Why: the parity corpus has a ±5-10% per-run variance band on the
# median ratio. A genuine 2-3% improvement from a hygiene fix sits
# under that band and looks like noise on any individual run. Three
# repeated full-corpus runs separated by a brief idle window let us
# report the median of the three, plus min/max for the spread.
#
# Usage:
#   scripts/perf/stable.sh <target-binary> <output-name> [REPS=3]
#
# Output:
#   target/perf/<output-name>-r{1..REPS}.jsonl   per-run JSONL
#   target/perf/<output-name>-stable.json        aggregated summary
#
# Environment passthrough: REDLINE_TESTING_BIN, REDLINEDB_*, PERF_*.

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
source "$(dirname "$0")/lib.sh"

target_bin="${1:?usage: stable.sh <target-binary> <output-name> [REPS=3]}"
out_name="${2:?usage: stable.sh <target-binary> <output-name> [REPS=3]}"
reps="${3:-3}"

if [ "$reps" -lt 1 ]; then
  printf 'stable.sh: REPS must be >= 1, got %s\n' "$reps" >&2
  exit 64
fi

perf_require_bins "$target_bin"

printf '==> stable.sh: %s × %d full-corpus runs -> target/perf/%s-stable.json\n' \
  "$target_bin" "$reps" "$out_name"

# Run the full corpus N times. We don't sleep between runs — fs caches stay
# warm which actually tightens the variance band (cold-cache first run was
# the source of the misleading A24=1.628 reading earlier in the campaign).
for i in $(seq 1 "$reps"); do
  printf '==> stable.sh: run %d of %d\n' "$i" "$reps"
  bash "$(dirname "$0")/full.sh" "$target_bin" "${out_name}-r${i}"
done

# Aggregate: per-case median ratios across all reps × cases, then report
# the case-level median / p90 / p95 / max / faster-count.
python3 - <<PY
import json, statistics, collections
from pathlib import Path

reps = $reps
out_name = "$out_name"
medians = []
p95s = []
maxs = []
fasters = []
per_run_summaries = []

for i in range(1, reps + 1):
    path = Path(f"target/perf/{out_name}-r{i}.jsonl")
    if not path.exists():
        continue
    by_case = collections.defaultdict(list)
    for line in path.open():
        line = line.strip()
        if not line:
            continue
        r = json.loads(line)
        if r.get("latency_ratio") is not None:
            cid = r.get("case_id") or r.get("id")
            by_case.setdefault(cid, []).append(r["latency_ratio"])
    case_medians = sorted(statistics.median(v) for v in by_case.values() if v)
    if not case_medians:
        continue
    n = len(case_medians)
    p95 = case_medians[int(n * 0.95)] if 0 < int(n * 0.95) < n else case_medians[-1]
    faster = sum(1 for r in case_medians if r < 1.0)
    medians.append(statistics.median(case_medians))
    p95s.append(p95)
    maxs.append(max(case_medians))
    fasters.append(faster)
    per_run_summaries.append({
        "run": i,
        "median": round(statistics.median(case_medians), 4),
        "p95": round(p95, 4),
        "max": round(max(case_medians), 4),
        "faster": faster,
    })

if not medians:
    print("stable.sh: no measurements produced", file=__import__("sys").stderr)
    exit(2)

summary = {
    "schema_version": "v0/stable/1",
    "reps": reps,
    "median_of_medians": round(statistics.median(medians), 4),
    "median_min": round(min(medians), 4),
    "median_max": round(max(medians), 4),
    "median_spread_pct": round(
        (max(medians) - min(medians)) / statistics.median(medians) * 100, 2
    ),
    "p95_of_p95s": round(statistics.median(p95s), 4),
    "max_of_maxs": round(max(maxs), 4),
    "faster_median": round(statistics.median(fasters), 4),
    "faster_range": [min(fasters), max(fasters)],
    "per_run": per_run_summaries,
}
out_path = Path(f"target/perf/{out_name}-stable.json")
out_path.write_text(json.dumps(summary, indent=2))
print(f"\n== stable summary ({reps} runs) ==")
print(f"  median-of-medians : {summary['median_of_medians']:.4f}× "
      f"(spread {summary['median_min']:.3f}–{summary['median_max']:.3f}, "
      f"±{summary['median_spread_pct']/2:.1f}%)")
print(f"  p95-of-p95s       : {summary['p95_of_p95s']:.4f}×")
print(f"  max-of-maxs       : {summary['max_of_maxs']:.4f}×")
print(f"  faster (median)   : {int(summary['faster_median'])} "
      f"(range {summary['faster_range'][0]}–{summary['faster_range'][1]})")
print(f"  per-run details   : {out_path}")
PY
