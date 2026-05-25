#!/usr/bin/env bash
# Generate the auto-metrics section of README.md.
#
# Reads:
#   target/redline-testing/all.jsonl   (parity run output)
#   target/jankurai/repo-score.json    (audit score)
#   tokei output                       (LOC: redlinedb vs sqlite reference)
#
# Writes (between `<!-- BEGIN: auto-generated:metrics -->` and the matching
# END marker in README.md): the metrics section with mermaid charts and
# ranked tables.
#
# Usage:
#   scripts/generate-metrics-readme.sh                # update README.md in place
#   scripts/generate-metrics-readme.sh --check        # exit 1 if README differs

set -euo pipefail

mode="update"
case "${1:-}" in
    "" | --update) mode="update" ;;
    --check)       mode="check" ;;
    -h | --help)
        cat <<EOF
usage: $0 [--update|--check]
  --update (default): rewrite the auto-generated metrics section in README.md
  --check           : exit non-zero if README.md does not match what --update
                      would produce. Used by CI to guard against stale README.
EOF
        exit 0
        ;;
    *)
        printf 'unknown flag: %s\n' "$1" >&2
        exit 64
        ;;
esac

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

readme="${repo_root}/README.md"
evidence_dir="${repo_root}/target/redline-testing"
all_jsonl="${evidence_dir}/all.jsonl"
sqlite_raw="${evidence_dir}/sqlite_parity.raw.jsonl"
beyond_raw="${evidence_dir}/beyond_sqlite.raw.jsonl"
memory_raw="${evidence_dir}/memory.raw.jsonl"
jankurai_json="${repo_root}/target/jankurai/repo-score.json"
sqlite_ref_dir="${repo_root}/target/sqlite-reference/source"
begin_marker='<!-- sqlite-parity-metrics:begin -->'
end_marker='<!-- sqlite-parity-metrics:end -->'

# Pick the richest input we have. The redlineDB CI pipeline writes the per-
# suite files plus the concatenated all.jsonl; partial runs may have only
# one of them.
parity_input=""
for candidate in "$sqlite_raw" "$all_jsonl"; do
    if [ -s "$candidate" ]; then
        parity_input="$candidate"
        break
    fi
done
if [ -z "$parity_input" ]; then
    printf 'generate-metrics: no parity JSONL under %s (tried sqlite_parity.raw.jsonl, all.jsonl)\n' \
        "$evidence_dir" >&2
    exit 1
fi

# Tokei output for the two engines under comparison. The SQLite reference
# source is fetched by scripts/sqlite/build-reference.sh; if the directory
# isn't populated we emit a "—" placeholder so the LOC table still renders.
redlinedb_loc_json="$(mktemp)"
sqlite_loc_json="$(mktemp)"
trap 'rm -f "$redlinedb_loc_json" "$sqlite_loc_json"' EXIT

tokei --types=Rust "$repo_root/crates" --output json >"$redlinedb_loc_json" 2>/dev/null || true
if [ -d "$sqlite_ref_dir" ]; then
    src_root="$(find "$sqlite_ref_dir" -maxdepth 1 -type d -name 'sqlite-autoconf-*' | head -n 1)"
    if [ -n "$src_root" ]; then
        tokei --types=C "$src_root/sqlite3.c" "$src_root/sqlite3.h" "$src_root/shell.c" \
            --output json >"$sqlite_loc_json" 2>/dev/null || true
    fi
fi

generated="$(
    REDLINEDB_LOC_JSON="$redlinedb_loc_json" \
    SQLITE_LOC_JSON="$sqlite_loc_json" \
    PARITY_INPUT="$parity_input" \
    BEYOND_INPUT="$beyond_raw" \
    MEMORY_INPUT="$memory_raw" \
    JANKURAI_JSON="$jankurai_json" \
    BEGIN_MARKER="$begin_marker" \
    END_MARKER="$end_marker" \
    python3 <<'PY'
import json
import os
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path

def load_jsonl(path_env: str):
    path_str = os.environ.get(path_env, "")
    if not path_str or not Path(path_str).is_file():
        return []
    out = []
    with open(path_str) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            out.append(json.loads(line))
    return out

def load_json(path_env: str):
    path_str = os.environ.get(path_env, "")
    if not path_str or not Path(path_str).is_file():
        return {}
    with open(path_str) as f:
        return json.load(f)

def shell_emoji(status: str) -> str:
    return {"passed": "✅", "skipped": "⏭️", "failed": "❌"}.get(status, "❓")

def median(xs):
    xs = [x for x in xs if x is not None]
    if not xs:
        return None
    return statistics.median(xs)

def measured_only(records):
    # Keep one record per (case_id, repetition_index, sample_role)
    # where sample_role starts with "measured" — that's the run that
    # carries latency/memory numbers (skips have role "skipped").
    return [r for r in records if str(r.get("sample_role", "")).startswith("measured")]

def fmt_int(n):
    if n is None:
        return "—"
    return f"{int(n):,}"

def fmt_ns(ns):
    if ns is None:
        return "—"
    ns = float(ns)
    if ns < 1_000:
        return f"{ns:.0f} ns"
    if ns < 1_000_000:
        return f"{ns/1_000:.2f} µs"
    if ns < 1_000_000_000:
        return f"{ns/1_000_000:.2f} ms"
    return f"{ns/1_000_000_000:.2f} s"

def fmt_ratio(r):
    if r is None:
        return "—"
    return f"{r:.2f}×"

def fmt_kb(kb):
    if kb is None:
        return "—"
    kb = float(kb)
    if kb < 1024:
        return f"{kb:.0f} KiB"
    return f"{kb/1024:.1f} MiB"

def safe(x, default=""):
    return default if x is None else x

# ---------- data ----------

parity_records = load_jsonl("PARITY_INPUT")
beyond_records = load_jsonl("BEYOND_INPUT")
memory_records = load_jsonl("MEMORY_INPUT")
jankurai = load_json("JANKURAI_JSON")
redlinedb_loc = load_json("REDLINEDB_LOC_JSON")
sqlite_loc = load_json("SQLITE_LOC_JSON")

# Suite status counts: for parity we need to dedupe by case_id (multiple
# repetitions per case → one status).
def status_per_case(records):
    by_case = {}
    for r in records:
        cid = r.get("case_id")
        if cid is None:
            continue
        status = r.get("status", "?")
        prev = by_case.get(cid)
        # "failed" wins over "passed"; "skipped" only if no other.
        if prev is None:
            by_case[cid] = status
        elif prev != "failed" and status == "failed":
            by_case[cid] = status
        elif prev == "skipped" and status in {"passed", "failed"}:
            by_case[cid] = status
    return Counter(by_case.values()), by_case

parity_status, parity_by_case = status_per_case(parity_records)
parity_total = sum(parity_status.values())
parity_passed = parity_status.get("passed", 0)
parity_skipped = parity_status.get("skipped", 0)
parity_failed = parity_status.get("failed", 0)

beyond_status, beyond_by_case = status_per_case(beyond_records)
beyond_total = sum(beyond_status.values())
beyond_passed = beyond_status.get("passed", 0)
beyond_skipped = beyond_status.get("skipped", 0)
beyond_failed = beyond_status.get("failed", 0)

memory_status, memory_by_case = status_per_case(memory_records)
memory_total = sum(memory_status.values())

# Latency: median target/reference elapsed across measured samples.
parity_measured = measured_only(parity_records)
parity_passed_measured = [r for r in parity_measured if r.get("status") == "passed"]

ref_ns = [r.get("reference_elapsed_ns") for r in parity_passed_measured]
tgt_ns = [r.get("target_elapsed_ns") for r in parity_passed_measured]
ratios = [r.get("latency_ratio") for r in parity_passed_measured if r.get("latency_ratio")]

median_ref = median(ref_ns)
median_tgt = median(tgt_ns)
median_ratio = median(ratios)

# Top-N slowest / fastest by latency_ratio across measured passed samples,
# grouped by case_id (take the median ratio across reps for each case).
case_ratios = defaultdict(list)
case_ref_ns = defaultdict(list)
case_tgt_ns = defaultdict(list)
case_name = {}
case_category = {}
for r in parity_passed_measured:
    cid = r.get("case_id")
    if cid is None:
        continue
    rr = r.get("latency_ratio")
    if rr is None:
        continue
    case_ratios[cid].append(rr)
    case_ref_ns[cid].append(r.get("reference_elapsed_ns"))
    case_tgt_ns[cid].append(r.get("target_elapsed_ns"))
    case_name[cid] = r.get("name", "?")
    case_category[cid] = r.get("category", "?")

case_median_ratio = {
    cid: median(vs) for cid, vs in case_ratios.items()
}
case_median_tgt = {cid: median(case_tgt_ns[cid]) for cid in case_ratios}
case_median_ref = {cid: median(case_ref_ns[cid]) for cid in case_ratios}

ranked_by_ratio = sorted(case_median_ratio.items(), key=lambda kv: kv[1])
fastest = ranked_by_ratio[:10]      # lowest ratio = redlinedb fastest relative to sqlite
slowest = list(reversed(ranked_by_ratio[-10:]))  # highest ratio = redlinedb slowest

# Memory: target_peak_rss_kb and reference_peak_rss_kb on the memory suite
# where status == passed (the binary captures RSS samples for the memory
# profile by default, no --memory-samples flag needed).
mem_measured = measured_only(memory_records)
mem_passed = [r for r in mem_measured if r.get("status") == "passed"]
case_tgt_rss = defaultdict(list)
case_ref_rss = defaultdict(list)
case_mem_name = {}
case_mem_cat = {}
for r in mem_passed:
    cid = r.get("case_id")
    tgt_rss = r.get("target_peak_rss_kb")
    ref_rss = r.get("reference_peak_rss_kb")
    if cid is None:
        continue
    if tgt_rss is not None:
        case_tgt_rss[cid].append(tgt_rss)
    if ref_rss is not None:
        case_ref_rss[cid].append(ref_rss)
    case_mem_name[cid] = r.get("name", "?")
    case_mem_cat[cid] = r.get("category", "?")
case_median_tgt_rss = {cid: median(vs) for cid, vs in case_tgt_rss.items()}
case_median_ref_rss = {cid: median(vs) for cid, vs in case_ref_rss.items()}
median_tgt_rss_kb = median(list(case_median_tgt_rss.values())) if case_median_tgt_rss else None
median_ref_rss_kb = median(list(case_median_ref_rss.values())) if case_median_ref_rss else None
median_rss_ratio = None
if median_tgt_rss_kb and median_ref_rss_kb:
    median_rss_ratio = median_tgt_rss_kb / median_ref_rss_kb

# Top-10 cases ranked by per-case redline/sqlite RSS ratio (biggest
# overhead first). Only consider cases where both engines reported RSS.
case_rss_ratio = {}
for cid, tgt in case_median_tgt_rss.items():
    ref = case_median_ref_rss.get(cid)
    if ref and ref > 0:
        case_rss_ratio[cid] = tgt / ref
ranked_by_rss_overhead = sorted(case_rss_ratio.items(), key=lambda kv: kv[1], reverse=True)
top_mem_overhead = ranked_by_rss_overhead[:10]

# Category-level pass rates (parity).
category_totals = Counter()
category_passes = Counter()
for cid, status in parity_by_case.items():
    cat = parity_records[0].get("category", "?") if parity_records else "?"
# Recompute properly per case.
case_status_with_cat = {}
case_cat = {}
for r in parity_records:
    cid = r.get("case_id")
    cat = r.get("category", "?")
    if cid is None:
        continue
    case_cat[cid] = cat
    case_status_with_cat[cid] = parity_by_case.get(cid, "?")
for cid, cat in case_cat.items():
    category_totals[cat] += 1
    if case_status_with_cat[cid] == "passed":
        category_passes[cat] += 1
# Only categories with at least 5 cases are meaningful for ranking
# (a single-case category trivially lands at 0% or 100%).
MIN_CAT_CASES = 5
significant_cats = {c: n for c, n in category_totals.items() if n >= MIN_CAT_CASES}
ranked_cats = sorted(
    significant_cats.items(),
    key=lambda kv: (category_passes[kv[0]] / kv[1] if kv[1] else 0, -kv[1]),
)
weakest_cats = ranked_cats[:5]      # lowest pass rate first
strongest_cats = list(reversed(ranked_cats[-5:]))

# Jankurai score.
raw_score = jankurai.get("raw_score") or jankurai.get("score") or jankurai.get("final_score")
final_score = jankurai.get("score") or jankurai.get("final_score") or raw_score
decision_doc = jankurai.get("decision") or {}
if isinstance(decision_doc, dict):
    decision = decision_doc.get("status") or decision_doc.get("decision") or "—"
    minimum_score = decision_doc.get("minimum_score")
else:
    decision = str(decision_doc) or "—"
    minimum_score = None
findings = jankurai.get("findings") or []
hard_findings = sum(1 for f in findings if str(f.get("severity", "")).lower() == "high")
medium_findings = sum(1 for f in findings if str(f.get("severity", "")).lower() == "medium")
low_findings = sum(1 for f in findings if str(f.get("severity", "")).lower() in {"low", "info"})
hard_caps = jankurai.get("caps_applied") or jankurai.get("hard_caps") or []
if isinstance(hard_caps, str):
    hard_caps = [hard_caps] if hard_caps else []

# LOC.
def loc_summary(loc_doc):
    if not loc_doc:
        return None
    # tokei root is {"Rust": {...}} or {"C": {...}}; take the only language.
    for lang, body in loc_doc.items():
        if lang == "Total":
            continue
        return {
            "lang": lang,
            "code": body.get("code"),
            "comments": body.get("comments"),
            "blanks": body.get("blanks"),
            "files": len(body.get("reports", [])),
        }
    return None

redlinedb_summary = loc_summary(redlinedb_loc)
sqlite_summary = loc_summary(sqlite_loc)

# ---------- markdown ----------

out_lines = []
push = out_lines.append

push(os.environ["BEGIN_MARKER"])
push("")
push("> _Auto-generated by `scripts/generate-metrics-readme.sh`. "
     "Do not edit by hand — CI regenerates this section on every push._")
push("")

# Headline status table.
def pct(p, t):
    return f"{(100*p/t):.1f}%" if t else "—"

push("## Test results")
push("")
push("| Suite | Total | Passed | Skipped | Failed | Pass rate |")
push("|---|---:|---:|---:|---:|---:|")
push(f"| SQLite parity (`sqlite_parity`) | {parity_total} | {parity_passed} | "
     f"{parity_skipped} | {parity_failed} | {pct(parity_passed, parity_total)} |")
push(f"| Memory (`memory`) | {memory_total} | "
     f"{memory_status.get('passed',0)} | {memory_status.get('skipped',0)} | "
     f"{memory_status.get('failed',0)} | "
     f"{pct(memory_status.get('passed',0), memory_total)} |")
push(f"| Beyond SQLite vs Postgres (`beyond_sqlite`) | {beyond_total} | "
     f"{beyond_passed} | {beyond_skipped} | {beyond_failed} | "
     f"{pct(beyond_passed, beyond_total)} |")
push("")

# Headline numbers.
push("## At a glance")
push("")
push("| Metric | RedlineDB | SQLite | Ratio (RedlineDB / SQLite) |")
push("|---|---:|---:|---:|")
push(f"| Median per-case latency (parity, passed only) | {fmt_ns(median_tgt)} | "
     f"{fmt_ns(median_ref)} | {fmt_ratio(median_ratio)} |")
push(f"| Median peak RSS (memory suite) | {fmt_kb(median_tgt_rss_kb)} | "
     f"{fmt_kb(median_ref_rss_kb)} | {fmt_ratio(median_rss_ratio)} |")
if redlinedb_summary and sqlite_summary:
    push(f"| Source LOC (code lines) | {fmt_int(redlinedb_summary['code'])} "
         f"({redlinedb_summary['lang']}) | {fmt_int(sqlite_summary['code'])} "
         f"({sqlite_summary['lang']}) | "
         f"{redlinedb_summary['code']/sqlite_summary['code']:.2f}× |")
else:
    push(f"| Source LOC (code lines) | "
         f"{fmt_int(redlinedb_summary['code']) if redlinedb_summary else '—'} | "
         f"{fmt_int(sqlite_summary['code']) if sqlite_summary else '—'} | — |")
if final_score is not None:
    push(f"| Jankurai score (final / raw, decision) | {final_score} / "
         f"{raw_score if raw_score is not None else '—'} ({decision}) | "
         f"— | — |")
push("")

# Per-category pass rates.
push("## Where RedlineDB is strong vs weak (by category)")
push("")
non_perfect = [
    (cat, total) for cat, total in significant_cats.items()
    if category_passes[cat] < total
]
non_perfect.sort(key=lambda kv: category_passes[kv[0]] / kv[1] if kv[1] else 0)
if non_perfect:
    push("Categories with at least one non-passing case "
         f"(showing all {len(non_perfect)} categories with ≥{MIN_CAT_CASES} cases that aren't 100% green):")
    push("")
    push("| Category | Cases | Passed | Pass rate |")
    push("|---|---:|---:|---:|")
    for cat, total in non_perfect[:10]:
        passed = category_passes[cat]
        push(f"| `{cat}` | {total} | {passed} | {pct(passed, total)} |")
    push("")
else:
    push(f"All {len(significant_cats)} categories with ≥{MIN_CAT_CASES} cases pass at 100%. "
         "Per-category latency rankings (below) show where RedlineDB is fast vs slow vs SQLite.")
    push("")

# Largest categories by case count — proves the corpus has scale.
top_by_size = sorted(significant_cats.items(), key=lambda kv: kv[1], reverse=True)[:8]
push("Largest categories by case count (corpus coverage):")
push("")
push("| Category | Cases | Passed | Pass rate |")
push("|---|---:|---:|---:|")
for cat, total in top_by_size:
    passed = category_passes[cat]
    push(f"| `{cat}` | {total} | {passed} | {pct(passed, total)} |")
push("")

# Latency mermaid chart — top 10 slowest cases (where target/reference ratio is highest).
if slowest:
    push("## Latency — top 10 slowest cases (RedlineDB / SQLite ratio)")
    push("")
    push("Higher bars = RedlineDB took longer relative to SQLite for that case. "
         "These are the rankings to investigate when squeezing per-case latency.")
    push("")
    push("```mermaid")
    push("xychart-beta")
    push('    title "Top 10 slowest cases (latency ratio)"')
    labels = ",".join(f'"{cid}"' for cid, _ in slowest)
    push(f"    x-axis [{labels}]")
    vals = ",".join(f"{r:.2f}" for _, r in slowest)
    max_y = max(r for _, r in slowest)
    push(f'    y-axis "Ratio" 0 --> {max_y + 0.5:.1f}')
    push(f"    bar [{vals}]")
    push("```")
    push("")
    push("| Case | Name | Category | Median target | Median SQLite | Ratio |")
    push("|---|---|---|---:|---:|---:|")
    for cid, ratio in slowest:
        push(f"| `{cid}` | {case_name.get(cid,'?')} | `{case_category.get(cid,'?')}` | "
             f"{fmt_ns(case_median_tgt.get(cid))} | "
             f"{fmt_ns(case_median_ref.get(cid))} | {fmt_ratio(ratio)} |")
    push("")

# Top 10 fastest — where RedlineDB beats SQLite.
if fastest:
    push("## Latency — top 10 fastest cases (RedlineDB / SQLite ratio)")
    push("")
    push("Lower bars = RedlineDB outpaced SQLite on that case. Wins to defend.")
    push("")
    push("```mermaid")
    push("xychart-beta")
    push('    title "Top 10 fastest cases (latency ratio)"')
    labels = ",".join(f'"{cid}"' for cid, _ in fastest)
    push(f"    x-axis [{labels}]")
    vals = ",".join(f"{r:.2f}" for _, r in fastest)
    max_y = max(r for _, r in fastest)
    push(f'    y-axis "Ratio" 0 --> {max_y + 0.5:.1f}')
    push(f"    bar [{vals}]")
    push("```")
    push("")
    push("| Case | Name | Category | Median target | Median SQLite | Ratio |")
    push("|---|---|---|---:|---:|---:|")
    for cid, ratio in fastest:
        push(f"| `{cid}` | {case_name.get(cid,'?')} | `{case_category.get(cid,'?')}` | "
             f"{fmt_ns(case_median_tgt.get(cid))} | "
             f"{fmt_ns(case_median_ref.get(cid))} | {fmt_ratio(ratio)} |")
    push("")

# Memory ranking — biggest RedlineDB-over-SQLite peak RSS overhead.
if top_mem_overhead:
    push("## Memory — top 10 cases by RSS overhead (RedlineDB / SQLite)")
    push("")
    push("Per-case peak resident set size (RSS) on the `memory` suite, ranked "
         "by overhead vs SQLite. Higher bars = RedlineDB held more memory for "
         "that case — the rankings to investigate when tightening allocator "
         "footprint.")
    push("")
    push("```mermaid")
    push("xychart-beta")
    push('    title "Top 10 cases by RSS overhead (ratio)"')
    labels = ",".join(f'"{cid}"' for cid, _ in top_mem_overhead)
    push(f"    x-axis [{labels}]")
    vals = ",".join(f"{v:.2f}" for _, v in top_mem_overhead)
    max_y = max(v for _, v in top_mem_overhead)
    push(f'    y-axis "Ratio" 0 --> {max_y * 1.1:.1f}')
    push(f"    bar [{vals}]")
    push("```")
    push("")
    push("| Case | Name | Category | RedlineDB RSS | SQLite RSS | Ratio |")
    push("|---|---|---|---:|---:|---:|")
    for cid, ratio in top_mem_overhead:
        push(f"| `{cid}` | {case_mem_name.get(cid,'?')} | "
             f"`{case_mem_cat.get(cid,'?')}` | "
             f"{fmt_kb(case_median_tgt_rss.get(cid))} | "
             f"{fmt_kb(case_median_ref_rss.get(cid))} | "
             f"{fmt_ratio(ratio)} |")
    push("")

# Beyond SQLite (Postgres-compared) feature coverage.
if beyond_records:
    # Beyond cases have feature-status / pass / fail / skip from the
    # oracle. Group by category for a per-feature breakdown.
    beyond_cat_total = Counter()
    beyond_cat_pass = Counter()
    for cid, status in beyond_by_case.items():
        # Look up category from the first matching record.
        cat = next((r.get("category", "?") for r in beyond_records if r.get("case_id") == cid), "?")
        beyond_cat_total[cat] += 1
        if status == "passed":
            beyond_cat_pass[cat] += 1
    push("## Beyond-SQLite features — vs Postgres oracle")
    push("")
    push(f"The `beyond_sqlite` suite runs the Postgres-shaped feature corpus "
         f"(`{beyond_total}` cases) against RedlineDB with Postgres as the "
         f"reference. RedlineDB is intentionally SQLite-shaped, so this suite "
         f"surfaces where the engine has been extended past the SQLite contract "
         f"and tracks divergence from Postgres semantics.")
    push("")
    push("| Outcome | Cases | Share |")
    push("|---|---:|---:|")
    push(f"| Passes Postgres semantics | {beyond_passed} | {pct(beyond_passed, beyond_total)} |")
    push(f"| Diverges (failing parity vs Postgres) | {beyond_failed} | {pct(beyond_failed, beyond_total)} |")
    push(f"| Skipped (Postgres-only / manifest backlog) | {beyond_skipped} | {pct(beyond_skipped, beyond_total)} |")
    push("")
    if beyond_cat_total:
        push("Top beyond-feature categories by coverage:")
        push("")
        push("| Category | Cases | Passing Postgres | Pass rate |")
        push("|---|---:|---:|---:|")
        for cat, total in sorted(beyond_cat_total.items(), key=lambda kv: kv[1], reverse=True)[:8]:
            push(f"| `{cat}` | {total} | {beyond_cat_pass[cat]} | "
                 f"{pct(beyond_cat_pass[cat], total)} |")
        push("")

# Jankurai detail.
push("## Code health (jankurai)")
push("")
if final_score is not None:
    min_str = f" (policy minimum `{minimum_score}`)" if minimum_score is not None else ""
    push(f"- **Score:** `{final_score}` (raw `{raw_score}`){min_str}")
    push(f"- **Decision:** `{decision}`")
    push(f"- **Hard findings (`high`):** {hard_findings}")
    push(f"- **Medium findings:** {medium_findings}")
    push(f"- **Low/info findings:** {low_findings}")
    if hard_caps:
        push(f"- **Hard rule caps applied:** {', '.join(f'`{c}`' for c in hard_caps)}")
    push("")

    # Dimensional breakdown — surfaces code-shape and other axes the user
    # care about ("strong vs weak"), sorted by weighted contribution so
    # the most impactful axis shows first.
    dimensions = jankurai.get("dimensions") or []
    if dimensions:
        push("Score breakdown by dimension (ranked by weighted contribution):")
        push("")
        push("| Dimension | Weight | Score | Weighted |")
        push("|---|---:|---:|---:|")
        sorted_dims = sorted(
            dimensions,
            key=lambda d: float(d.get("weighted_points", 0)),
            reverse=True,
        )
        for d in sorted_dims:
            push(f"| {d.get('name','?')} | {d.get('weight',0)} | "
                 f"{float(d.get('score',0)):.1f} | "
                 f"{float(d.get('weighted_points',0)):.1f} |")
        push("")

    push("See `target/jankurai/repo-score.md` for the full report — the CI "
         "`audit-ci` lane fails if score drops below the policy minimum or "
         "any hard cap is applied.")
else:
    push("Jankurai report not available locally. The CI `audit-ci` lane "
         "publishes `target/jankurai/repo-score.json`.")
push("")

# LOC detail.
push("## Lines of code")
push("")
if redlinedb_summary or sqlite_summary:
    push("| Engine | Language | Code lines | Comments | Files |")
    push("|---|---|---:|---:|---:|")
    if redlinedb_summary:
        push(f"| RedlineDB (`crates/`) | {redlinedb_summary['lang']} | "
             f"{fmt_int(redlinedb_summary['code'])} | "
             f"{fmt_int(redlinedb_summary['comments'])} | "
             f"{fmt_int(redlinedb_summary['files'])} |")
    if sqlite_summary:
        push(f"| SQLite ({sqlite_summary['lang']}) amalgamation | "
             f"{sqlite_summary['lang']} | "
             f"{fmt_int(sqlite_summary['code'])} | "
             f"{fmt_int(sqlite_summary['comments'])} | "
             f"{fmt_int(sqlite_summary['files'])} |")
    if redlinedb_summary and sqlite_summary:
        delta = redlinedb_summary['code'] - sqlite_summary['code']
        push(f"| **Delta (RedlineDB − SQLite)** | — | "
             f"{('+' if delta >= 0 else '')}{fmt_int(delta)} | — | — |")
    push("")

push(os.environ["END_MARKER"])

print("\n".join(out_lines))
PY
)"

if [ -z "$generated" ]; then
    printf 'generate-metrics: generator produced no output\n' >&2
    exit 1
fi

# Splice the generated block between the markers in README.md.
if [ ! -f "$readme" ]; then
    printf 'generate-metrics: README.md missing at %s\n' "$readme" >&2
    exit 1
fi

awk -v begin_marker="$begin_marker" -v end_marker="$end_marker" \
    -v generated="$generated" '
    BEGIN { in_block = 0 }
    {
        if ($0 == begin_marker) {
            print generated
            in_block = 1
            next
        }
        if (in_block && $0 == end_marker) {
            in_block = 0
            next
        }
        if (!in_block) {
            print $0
        }
    }
' "$readme" >"${readme}.tmp"

# If the markers were missing the awk pass leaves the original content
# unchanged and the generated block is lost. Detect that case explicitly
# so the README stays self-healing.
if ! grep -qF "$begin_marker" "${readme}.tmp"; then
    printf 'generate-metrics: README is missing the auto-generated marker block:\n'
    printf '  %s\n  %s\n' "$begin_marker" "$end_marker"
    printf 'append the marker pair to README.md (any order) and rerun.\n'
    rm -f "${readme}.tmp"
    exit 1
fi >&2

case "$mode" in
    update)
        mv "${readme}.tmp" "$readme"
        printf 'generate-metrics: README.md updated.\n' >&2
        ;;
    check)
        if ! diff -u "$readme" "${readme}.tmp" >/dev/null; then
            printf 'generate-metrics: README.md is stale — re-run `bash scripts/generate-metrics-readme.sh` and commit.\n' >&2
            diff -u "$readme" "${readme}.tmp" >&2 || true
            rm -f "${readme}.tmp"
            exit 1
        fi
        rm -f "${readme}.tmp"
        printf 'generate-metrics: README.md is current.\n' >&2
        ;;
esac
