#!/usr/bin/env python3
"""Compare two redline-testing JSONL outputs.

Usage:
  scripts/perf/diff.py <before.jsonl> <after.jsonl>
    [--top N] [--full] [--regression-threshold PCT]

Computes per-case median target_elapsed_ns and latency_ratio in each
run, joins on case_id, emits:
  1. Markdown table of cases sorted by abs(delta_pct) descending.
     (--top N caps to N rows; --full emits every common case.)
  2. Summary footer: median delta%, median ratio change, sign test,
     worst regression, best improvement.

Both inputs should come from the SAME case set; the non-intersection is
counted in the footer but not displayed.

Pure stdlib (json, statistics, csv, argparse).
"""

import argparse
import json
import statistics
import sys
from collections import defaultdict


def load_jsonl(path):
    per_case = defaultdict(
        lambda: {"target": [], "sqlite": [], "ratio": [], "meta": {}}
    )
    with open(path) as f:
        for line in f:
            try:
                d = json.loads(line)
            except json.JSONDecodeError:
                continue
            if d.get("status") != "passed":
                continue
            role = d.get("sample_role")
            if not (isinstance(role, str) and role.startswith("measured")):
                continue
            try:
                target_ns = float(d["target_elapsed_ns"])
                sqlite_ns = float(d["reference_elapsed_ns"])
                ratio = float(d["latency_ratio"])
            except (KeyError, TypeError, ValueError):
                continue
            cid = d["case_id"]
            per_case[cid]["target"].append(target_ns)
            per_case[cid]["sqlite"].append(sqlite_ns)
            per_case[cid]["ratio"].append(ratio)
            per_case[cid]["meta"] = {
                k: d.get(k, "") for k in ("name", "priority", "profile", "category")
            }
    summary = {}
    for cid, v in per_case.items():
        if not v["target"]:
            continue
        summary[cid] = {
            "target_ns": statistics.median(v["target"]),
            "sqlite_ns": statistics.median(v["sqlite"]),
            "ratio": statistics.median(v["ratio"]),
            "n": len(v["target"]),
            "meta": v["meta"],
        }
    return summary


def main():
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("before")
    ap.add_argument("after")
    ap.add_argument(
        "--top", type=int, default=30, help="rows to display (default 30)"
    )
    ap.add_argument(
        "--full",
        action="store_true",
        help="show all cases, ignoring --top",
    )
    ap.add_argument(
        "--regression-threshold",
        type=float,
        default=5.0,
        help="percent slower to flag as regressed (default 5.0)",
    )
    args = ap.parse_args()

    b = load_jsonl(args.before)
    a = load_jsonl(args.after)
    common = sorted(set(b) & set(a))
    only_b = sorted(set(b) - set(a))
    only_a = sorted(set(a) - set(b))

    deltas = []
    for cid in common:
        before_ns = b[cid]["target_ns"]
        after_ns = a[cid]["target_ns"]
        if before_ns <= 0:
            continue
        delta_pct = (after_ns - before_ns) / before_ns * 100.0
        deltas.append(
            {
                "cid": cid,
                "name": b[cid]["meta"].get("name", ""),
                "category": b[cid]["meta"].get("category", ""),
                "before_ns": before_ns,
                "after_ns": after_ns,
                "delta_pct": delta_pct,
                "ratio_before": b[cid]["ratio"],
                "ratio_after": a[cid]["ratio"],
            }
        )

    deltas_by_impact = sorted(deltas, key=lambda r: -abs(r["delta_pct"]))
    display = deltas_by_impact if args.full else deltas_by_impact[: args.top]

    print(f"# Perf diff: {args.before} -> {args.after}")
    print()
    print("| case_id | name | category | before ms | after ms | delta% | ratio before | ratio after |")
    print("|---|---|---|---:|---:|---:|---:|---:|")
    for r in display:
        sign = "+" if r["delta_pct"] >= 0 else ""
        print(
            f"| {r['cid']} | {r['name']} | {r['category']} | "
            f"{r['before_ns'] / 1e6:.2f} | {r['after_ns'] / 1e6:.2f} | "
            f"{sign}{r['delta_pct']:.2f} | "
            f"{r['ratio_before']:.2f} | {r['ratio_after']:.2f} |"
        )

    print()
    print("## Summary")
    print(f"- cases compared: {len(common)} (only-before: {len(only_b)}, only-after: {len(only_a)})")
    if not deltas:
        print("- no measurable common cases")
        return
    median_pct = statistics.median(r["delta_pct"] for r in deltas)
    median_ratio_b = statistics.median(r["ratio_before"] for r in deltas)
    median_ratio_a = statistics.median(r["ratio_after"] for r in deltas)
    improved = [r for r in deltas if r["delta_pct"] < -args.regression_threshold]
    regressed = [r for r in deltas if r["delta_pct"] > args.regression_threshold]
    faster = sum(1 for r in deltas if r["delta_pct"] < 0)
    slower = sum(1 for r in deltas if r["delta_pct"] > 0)
    print(f"- median per-case delta: {median_pct:+.2f}%")
    print(
        f"- median ratio: {median_ratio_b:.3f} -> {median_ratio_a:.3f}  "
        f"(delta {median_ratio_a - median_ratio_b:+.3f})"
    )
    print(f"- improved >{args.regression_threshold}%: {len(improved)} cases")
    print(f"- regressed >{args.regression_threshold}%: {len(regressed)} cases")
    print(f"- sign test: {faster} faster vs {slower} slower")
    if regressed:
        worst = max(regressed, key=lambda r: r["delta_pct"])
        print(f"- worst regression: {worst['cid']} {worst['name']} {worst['delta_pct']:+.2f}%")
    if improved:
        best = min(improved, key=lambda r: r["delta_pct"])
        print(f"- best improvement: {best['cid']} {best['name']} {best['delta_pct']:+.2f}%")

    # Exit code: nonzero if any regression above threshold (CI gate hook)
    sys.exit(1 if regressed else 0)


if __name__ == "__main__":
    main()
