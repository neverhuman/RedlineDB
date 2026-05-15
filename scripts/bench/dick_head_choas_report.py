#!/usr/bin/env python3
"""Summarize chaos benchmark raw records and persist versioned JSON."""

from __future__ import annotations

import argparse
import collections
import copy
import glob
import json
import statistics
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_VERSION_ROOT = REPO_ROOT / "target" / "bench" / "versioned"
DEFAULT_SUITE = "dick-head-choas"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True, help="xbabe1 benchmark stamp directory")
    parser.add_argument(
        "--version-root",
        default=str(DEFAULT_VERSION_ROOT),
        help="root directory for git-hash keyed JSON exports",
    )
    parser.add_argument(
        "--suite",
        default=DEFAULT_SUITE,
        help="suite name recorded in the exported JSON bundle",
    )
    return parser.parse_args()


def load_records(stamp_dir: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for path in sorted(glob.glob(str(stamp_dir / "raw" / "*" / "record.json"))):
        with open(path, "r", encoding="utf-8") as handle:
            record = json.load(handle)
        record["_path"] = path
        record["_run"] = Path(path).parent.name
        records.append(record)
    return records


def load_manifest(stamp_dir: Path) -> dict[str, Any] | None:
    path = stamp_dir / "manifest.json"
    if not path.exists():
        return None
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def median(values: Iterable[float]) -> float:
    values = list(values)
    return statistics.median(values) if values else float("nan")


def repo_git_sha() -> str:
    return subprocess.check_output(
        ["git", "-C", str(REPO_ROOT), "rev-parse", "HEAD"],
        text=True,
    ).strip()


def infer_source_paths(workload: str) -> list[str]:
    chaos = {
        "chaos-lock-convoy",
        "chaos-connection-churn",
        "chaos-checkpoint-thrash",
        "chaos-index-hammer",
        "chaos-temp-spill-convoy",
        "chaos-schema-storm",
    }
    index_batch = {
        "secondary-index-range",
        "secondary-index-count",
    }
    index_access = {
        "secondary-index-read",
        "covered-range-cold",
        "covered-range-warm",
    }
    top = {"secondary-index-ordered-limit"}
    workload_rs = {
        "single-row-insert",
        "batched-insert100",
        "point-read-pk",
        "writers-disjoint",
        "hot-row-update",
        "mixed-oltp",
        "mixed95-read5-write",
        "mixed80-read20-write",
        "mixed50-read50-write",
        "connection-limit",
        "large-sort-spill",
        "json-path-extract",
        "json-path-update",
        "vector-flat-search",
        "vector-ann-search",
        "vector-ann-search-disk",
        "commit-storm-batched",
        "hot-counter-update",
        "queue-mixed",
    }
    source_paths = []
    if workload in chaos:
        source_paths.append("crates/bench/src/chaos.rs")
    elif workload in index_batch:
        source_paths.append("crates/sql/src/exec/index_batch.rs")
    elif workload in index_access:
        source_paths.append("crates/sql/src/exec/index_access.rs")
    elif workload in top:
        source_paths.append("crates/sql/src/exec/select_top.rs")
    elif workload in workload_rs:
        source_paths.append("crates/bench/src/workload.rs")
    else:
        source_paths.append("crates/bench/src/workload.rs")
    return source_paths


def normalize_record(record: dict[str, Any], manifest: dict[str, Any] | None) -> dict[str, Any]:
    out = copy.deepcopy(record)
    workload = str(record.get("workload", ""))
    engine_stats = record.get("engine_stats", {})
    source_paths = []
    if isinstance(engine_stats, dict):
        raw_source = engine_stats.get("test_code_path")
        if isinstance(raw_source, str) and raw_source:
            source_paths.append(raw_source)
    if not source_paths:
        source_paths = infer_source_paths(workload)
    out["raw_path"] = record["_path"]
    out["run_dir"] = record["_run"]
    out["test_code_paths"] = sorted(set(source_paths))
    if manifest and isinstance(manifest.get("config_path"), str):
        out["config_path"] = manifest["config_path"]
    return out


def record_source_paths(record: dict[str, Any]) -> list[str]:
    engine_stats = record.get("engine_stats", {})
    if isinstance(engine_stats, dict):
        raw_source = engine_stats.get("test_code_path")
        if isinstance(raw_source, str) and raw_source:
            return [raw_source]
    return infer_source_paths(str(record.get("workload", "")))


def engine_summary(runs: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "runs": len(runs),
        "median_qps": median(run["metrics"]["throughput_ops_per_sec"] for run in runs),
        "median_p99_us": median(run["metrics"]["latency"]["p99_us"] for run in runs),
        "median_failures": median(run["metrics"]["failures"] for run in runs),
        "median_busy_errors": median(run["metrics"]["busy_errors"] for run in runs),
        "median_locked_errors": median(run["metrics"]["locked_errors"] for run in runs),
        "median_timeout_errors": median(run["metrics"]["timeout_errors"] for run in runs),
        "median_elapsed_ms": median(run["metrics"]["elapsed_ms"] for run in runs),
        "raw_record_paths": [run["_path"] for run in runs],
        "run_ids": [run["run_id"] for run in runs],
    }


def build_comparisons(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str, str, int], list[dict[str, Any]]] = collections.defaultdict(list)
    for record in records:
        if str(record["_run"]).endswith("-w0"):
            continue
        key = (
            str(record["engine"]),
            str(record["workload"]),
            str(record["durability"]),
            int(record["threads"]),
        )
        grouped[key].append(record)

    by_workload: dict[tuple[str, str, int], dict[str, Any]] = {}
    for key, runs in grouped.items():
        engine, workload, durability, threads = key
        by_workload.setdefault((workload, durability, threads), {})[engine] = runs

    comparisons: list[dict[str, Any]] = []
    for (workload, durability, threads), engines in sorted(by_workload.items()):
        redline_runs = engines.get("redline", [])
        sqlite_runs = engines.get("sqlite", [])
        source_paths = sorted(
            {
                path
                for run in redline_runs + sqlite_runs
                for path in record_source_paths(run)
            }
        )
        comparison: dict[str, Any] = {
            "workload": workload,
            "durability": durability,
            "threads": threads,
            "test_code_paths": source_paths,
        }
        if redline_runs:
            comparison["redline"] = engine_summary(redline_runs)
        if sqlite_runs:
            comparison["sqlite"] = engine_summary(sqlite_runs)
        if redline_runs and sqlite_runs:
            redline_qps = comparison["redline"]["median_qps"]
            sqlite_qps = comparison["sqlite"]["median_qps"]
            comparison["ratio_redline_vs_sqlite"] = (
                redline_qps / sqlite_qps if sqlite_qps else None
            )
        comparisons.append(comparison)
    return comparisons


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")


def refresh_index(version_dir: Path, git_sha: str) -> Path:
    reports: list[dict[str, Any]] = []
    for path in sorted(version_dir.glob("*.json")):
        if path.name == "index.json":
            continue
        with open(path, "r", encoding="utf-8") as handle:
            data = json.load(handle)
        reports.append(
            {
                "file": path.name,
                "stamp": data.get("stamp"),
                "suite": data.get("suite"),
                "profile": data.get("profile"),
                "workloads": data.get("workloads", []),
                "config_path": data.get("config_path"),
            }
        )
    index = {
        "git_sha": git_sha,
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "reports": reports,
    }
    index_path = version_dir / "index.json"
    write_json(index_path, index)
    return index_path


def main() -> int:
    args = parse_args()
    stamp_dir = Path(args.input)
    version_root = Path(args.version_root)
    manifest = load_manifest(stamp_dir)
    records = load_records(stamp_dir)
    if not records:
        print(f"no records found under {stamp_dir}")
        return 1

    grouped = collections.defaultdict(list)
    for record in records:
        if str(record["_run"]).endswith("-w0"):
            continue
        key = (
            str(record["engine"]),
            str(record["workload"]),
            str(record["durability"]),
            int(record["threads"]),
        )
        grouped[key].append(record)

    git_sha = None
    if manifest and isinstance(manifest.get("git_sha"), str):
        git_sha = manifest["git_sha"]
    if not git_sha:
        git_sha = repo_git_sha()

    version_dir = version_root / git_sha
    version_dir.mkdir(parents=True, exist_ok=True)

    print(f"stamp: {stamp_dir.name}")
    print(f"records: {len(records)}")
    for key in sorted(grouped):
        engine, workload, durability, threads = key
        runs = grouped[key]
        qps = [run["metrics"]["throughput_ops_per_sec"] for run in runs]
        p99 = [run["metrics"]["latency"]["p99_us"] for run in runs]
        failures = [run["metrics"]["failures"] for run in runs]
        busy = [run["metrics"]["busy_errors"] for run in runs]
        locked = [run["metrics"]["locked_errors"] for run in runs]
        timeout = [run["metrics"]["timeout_errors"] for run in runs]
        print(
            f"{engine:7} {workload:38} {durability:7} t{threads:<4} "
            f"qps={median(qps):10.2f} p99_us={median(p99):8.0f} "
            f"fail={median(failures):4.0f} busy={median(busy):4.0f} "
            f"locked={median(locked):4.0f} timeout={median(timeout):4.0f}"
        )

    print()
    print("redline-vs-sqlite median qps ratios:")
    comparisons = build_comparisons(records)
    for comparison in comparisons:
        redline = comparison.get("redline")
        sqlite = comparison.get("sqlite")
        if redline and sqlite:
            ratio = comparison["ratio_redline_vs_sqlite"]
            ratio_display = f"{ratio:8.3f}" if isinstance(ratio, (int, float)) else "   null"
            print(
                f"{comparison['workload']:38} {comparison['durability']:7} t{comparison['threads']:<4} "
                f"ratio={ratio_display} redline={redline['median_qps']:10.2f} "
                f"sqlite={sqlite['median_qps']:10.2f}"
            )

    normalized_records = [normalize_record(record, manifest) for record in records]
    report = {
        "schema_version": 1,
        "suite": manifest.get("suite", args.suite) if manifest else args.suite,
        "stamp": stamp_dir.name,
        "git_sha": git_sha,
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "input_dir": str(stamp_dir),
        "config_path": manifest.get("config_path") if manifest else None,
        "config_hash": manifest.get("config_hash") if manifest else None,
        "workloads": sorted({str(record["workload"]) for record in records}),
        "source_paths": sorted(
            {
                path
                for record in normalized_records
                for path in record.get("test_code_paths", [])
            }
        ),
        "records": normalized_records,
        "comparisons": comparisons,
        "manifest": manifest,
    }

    stamp_report_path = stamp_dir / "versioned-results.json"
    write_json(stamp_report_path, report)

    version_report_path = version_dir / f"{stamp_dir.name}.json"
    write_json(version_report_path, report)
    index_path = refresh_index(version_dir, git_sha)

    print()
    print(f"versioned_json: {version_report_path}")
    print(f"stamp_json: {stamp_report_path}")
    print(f"version_index: {index_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
