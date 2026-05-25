#!/usr/bin/env python3
"""Build deterministic perf case-lists from the redline-testing corpus.

Reads:
  - --snapshot       JSON list from `redline-testing list --suite sqlite_parity --format json`
  - --ranked         benchmark-results/sqlite-parity/latest/ranked.csv (CSV header documented below)

Writes:
  - --quick-out      ~40 case IDs for sub-5min A/B iteration
  - --medium-out     ~300 case IDs for the PR-gate sweep

Quick-set composition:
  * Top 15 regressions (worst latency ratio first)
  * 2 per major category (worst + median in that category)
  * 5 canary cases where redlinedb is currently FASTER than sqlite
  * 4 profile-coverage picks (one per: tempfile, catalog, side_effect, external_app)

Medium-set composition:
  * Every P0 case (~130)
  * The 100 worst-regressed P1 cases
  * 5 distribution-spread picks per major category (worst, p75, median, p25, best)
  * Top 30 absolute-latency outliers (highest target_elapsed_ns)

Output format matches crates/bench/src/sqlite_parity/cli.rs::parse_case_list_text:
  one 5-digit case-id per line, '#' comments allowed.
"""

import argparse
import csv
import json
import statistics
import sys
from collections import defaultdict

MAJOR_CATEGORIES = [
    "GEN_SQL_DML",
    "GEN_SQL_SCALAR",
    "GEN_SQL_AGGREGATE",
    "GEN_SQL_WINDOW",
    "GEN_SQL_CTE",
    "GEN_SQL_JSON",
    "GEN_SQL_INDEX_PRAGMA",
    "GEN_SQL_CONSTRAINT_TX",
    "GEN_SQL_JOIN_SUBQUERY",
    "GEN_SQL_VIEW_TRIGGER",
]

# Cases that redline-testing v1.0.0+ skips entirely; including them in a
# perf case-list produces "skipped" samples with no comparison value.
SKIPPED_IDS = {"00093", "00094", "00095", "00096"}


def parse_args():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--snapshot", required=True, help="JSON output of `redline-testing list`")
    p.add_argument("--ranked", required=True, help="benchmark-results/.../ranked.csv")
    p.add_argument("--quick-out", required=True)
    p.add_argument("--medium-out", required=True)
    return p.parse_args()


def load_ranked(path):
    with open(path, newline="") as f:
        rows = list(csv.DictReader(f))
    for r in rows:
        try:
            sqlite_ns = float(r["sqlite_median_ns"]) if r["sqlite_median_ns"] else 0.0
            redline_ns = float(r["redline_median_ns"]) if r["redline_median_ns"] else 0.0
        except ValueError:
            sqlite_ns = redline_ns = 0.0
        r["_ratio"] = (redline_ns / sqlite_ns) if sqlite_ns > 0 else float("nan")
        r["_target_ns"] = redline_ns
    return rows


def main():
    args = parse_args()
    with open(args.snapshot) as f:
        snapshot = json.load(f)
    if not isinstance(snapshot, list):
        print("snapshot: expected JSON list at top level", file=sys.stderr)
        sys.exit(2)

    valid_ids = {f"{c['id']:05d}" for c in snapshot}
    ranked = [
        r for r in load_ranked(args.ranked)
        if r["case_id"] not in SKIPPED_IDS
        and r["case_id"] in valid_ids
        and r["_ratio"] == r["_ratio"]  # not NaN
    ]
    by_cat = defaultdict(list)
    for r in ranked:
        if r["category"] in MAJOR_CATEGORIES:
            by_cat[r["category"]].append(r)

    # ===== QUICK SET =====
    quick = []
    chosen_quick = set()

    def add_quick(cid, note):
        if cid not in chosen_quick:
            quick.append((cid, note))
            chosen_quick.add(cid)

    by_ratio_desc = sorted(ranked, key=lambda r: -r["_ratio"])
    for r in by_ratio_desc[:15]:
        add_quick(
            r["case_id"],
            f"regression rank-{len(quick) + 1} ratio={r['_ratio']:.2f} {r['name']}",
        )

    for cat in MAJOR_CATEGORIES:
        rows = sorted(by_cat[cat], key=lambda r: -r["_ratio"])
        if not rows:
            continue
        # worst-in-category + median-in-category
        for picked in (rows[0], rows[len(rows) // 2]):
            add_quick(
                picked["case_id"],
                f"category {cat} ratio={picked['_ratio']:.2f} {picked['name']}",
            )

    # canaries: 5 cases where we are FASTEST relative to sqlite
    canaries = sorted(ranked, key=lambda r: r["_ratio"])[:5]
    for r in canaries:
        add_quick(
            r["case_id"],
            f"canary FASTER ratio={r['_ratio']:.2f} {r['name']}",
        )

    for profile in ("tempfile", "catalog", "side_effect", "external_app"):
        rows = [r for r in ranked if r["profile"] == profile]
        if not rows:
            continue
        picked = sorted(rows, key=lambda r: -r["_ratio"])[0]
        add_quick(
            picked["case_id"],
            f"profile {profile} ratio={picked['_ratio']:.2f} {picked['name']}",
        )

    # ===== MEDIUM SET =====
    medium_ids = set()
    medium_notes = {}

    def add_medium(cid, note):
        if cid not in medium_ids:
            medium_ids.add(cid)
            medium_notes[cid] = note

    for r in ranked:
        if r["priority"] == "P0":
            add_medium(r["case_id"], f"P0 {r['category']} {r['name']}")

    p1_worst = sorted(
        [r for r in ranked if r["priority"] == "P1"], key=lambda r: -r["_ratio"]
    )[:100]
    for r in p1_worst:
        add_medium(
            r["case_id"],
            f"P1-worst ratio={r['_ratio']:.2f} {r['category']} {r['name']}",
        )

    for cat in MAJOR_CATEGORIES:
        rows = sorted(by_cat[cat], key=lambda r: -r["_ratio"])
        if len(rows) < 5:
            for r in rows:
                add_medium(r["case_id"], f"cat-spread {cat} ratio={r['_ratio']:.2f} {r['name']}")
            continue
        idxs = [0, len(rows) // 4, len(rows) // 2, 3 * len(rows) // 4, len(rows) - 1]
        for i in idxs:
            r = rows[i]
            add_medium(
                r["case_id"],
                f"cat-spread {cat} ratio={r['_ratio']:.2f} {r['name']}",
            )

    abs_outliers = sorted(ranked, key=lambda r: -r["_target_ns"])[:30]
    for r in abs_outliers:
        add_medium(
            r["case_id"],
            f"abs-outlier {r['_target_ns'] / 1e6:.1f}ms {r['name']}",
        )

    def write_list(path, items):
        items = sorted(set(items), key=lambda x: x[0])
        with open(path, "w") as f:
            f.write("# Auto-generated by scripts/perf/build_case_lists.py\n")
            f.write(f"# corpus_size={len(snapshot)} cases_selected={len(items)}\n")
            for cid, note in items:
                f.write(f"{cid}  # {note}\n")

    write_list(args.quick_out, quick)
    write_list(args.medium_out, [(cid, medium_notes[cid]) for cid in medium_ids])

    print(f"quick:  {len(quick)} cases -> {args.quick_out}")
    print(f"medium: {len(medium_ids)} cases -> {args.medium_out}")


if __name__ == "__main__":
    main()
