#!/usr/bin/env python3
"""Export benchmark results into a git-hash keyed version folder."""

from __future__ import annotations

import argparse
import collections
import copy
import glob
import hashlib
import json
import os
import shutil
import statistics
import subprocess
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_INPUT_ROOT = REPO_ROOT / "target" / "bench"
DEFAULT_VERSION_ROOT = REPO_ROOT / "benchmark-results" / "version"


@dataclass(frozen=True)
class SuiteSpec:
    suite_id: str
    kind: str
    input_path: Path
    command: list[str]
    config_path: str | None
    source_files: list[str]
    extra_files: list[str] = ()


SUITES: list[SuiteSpec] = [
    SuiteSpec(
        suite_id="phase9-smoke",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "certify-smoke",
        command=[
            "rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/smoke.toml --out-dir target/bench/certify-smoke --seed 7 --repetitions 1 --warmup 0",
        ],
        config_path="crates/bench/bench/smoke.toml",
        source_files=[
            "crates/bench/src/certify.rs",
            "crates/bench/src/workload.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/engine/sqlite.rs",
            "crates/bench/src/config.rs",
        ],
    ),
    SuiteSpec(
        suite_id="phase9-certification",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "certify-certification",
        command=[
            "rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/certification.toml --out-dir target/bench/certify-certification --seed 7 --repetitions 5 --warmup 1",
        ],
        config_path="crates/bench/bench/certification.toml",
        source_files=[
            "crates/bench/src/certify.rs",
            "crates/bench/src/workload.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/engine/sqlite.rs",
            "crates/bench/src/config.rs",
        ],
    ),
    SuiteSpec(
        suite_id="phase9-xbabe1-gap",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "xbabe1" / "gap-cert",
        command=[
            "./scripts/bench/xbabe1_sync.sh",
            "./scripts/bench/xbabe1_run.sh rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/gap-cert.toml --out-dir target/bench/xbabe1/gap-cert --seed 7 --repetitions 3 --warmup 1",
            "./scripts/bench/xbabe1_fetch.sh gap-cert",
        ],
        config_path="crates/bench/bench/gap-cert.toml",
        source_files=[
            "crates/bench/src/certify.rs",
            "crates/bench/src/workload.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/engine/sqlite.rs",
            "crates/bench/src/config.rs",
        ],
    ),
    SuiteSpec(
        suite_id="phase9-xbabe1-gap-strace",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "xbabe1" / "gap-cert-strace",
        command=[
            "./scripts/bench/xbabe1_sync.sh",
            "./scripts/bench/xbabe1_run.sh rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/gap-cert.toml --out-dir target/bench/xbabe1/gap-cert-strace --seed 7 --repetitions 3 --warmup 1 --with-strace",
            "./scripts/bench/xbabe1_fetch.sh gap-cert-strace",
        ],
        config_path="crates/bench/bench/gap-cert.toml",
        source_files=[
            "crates/bench/src/certify.rs",
            "crates/bench/src/workload.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/engine/sqlite.rs",
            "crates/bench/src/config.rs",
        ],
    ),
    SuiteSpec(
        suite_id="phase9-xbabe1-certification",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "xbabe1" / "certification",
        command=[
            "./scripts/bench/xbabe1_sync.sh",
            "./scripts/bench/xbabe1_run.sh rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/certification.toml --out-dir target/bench/xbabe1/certification --seed 7 --repetitions 5 --warmup 1",
            "./scripts/bench/xbabe1_fetch.sh certification",
        ],
        config_path="crates/bench/bench/certification.toml",
        source_files=[
            "crates/bench/src/certify.rs",
            "crates/bench/src/workload.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/engine/sqlite.rs",
            "crates/bench/src/config.rs",
        ],
    ),
    SuiteSpec(
        suite_id="phase9-xbabe1-certify-with-strace",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "xbabe1" / "certify-strace",
        command=[
            "./scripts/bench/xbabe1_sync.sh",
            "./scripts/bench/xbabe1_run.sh rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/certification.toml --out-dir target/bench/xbabe1/certify-strace --seed 7 --repetitions 5 --warmup 1 --with-strace",
            "./scripts/bench/xbabe1_fetch.sh certify-strace",
        ],
        config_path="crates/bench/bench/certification.toml",
        source_files=[
            "crates/bench/src/certify.rs",
            "crates/bench/src/workload.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/engine/sqlite.rs",
            "crates/bench/src/config.rs",
        ],
    ),
    SuiteSpec(
        suite_id="phase10-xbabe1-certification",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "xbabe1" / "phase10-cert",
        command=[
            "./scripts/bench/xbabe1_sync.sh",
            "./scripts/bench/xbabe1_run.sh cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/certification.toml --out-dir target/bench/xbabe1/phase10-cert --seed 7 --repetitions 5 --warmup 1",
            "./scripts/bench/xbabe1_fetch.sh phase10-cert",
        ],
        config_path="crates/bench/bench/certification.toml",
        source_files=[
            "crates/bench/src/certify.rs",
            "crates/bench/src/workload.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/engine/sqlite.rs",
            "crates/bench/src/config.rs",
        ],
    ),
    SuiteSpec(
        suite_id="phase10-v3-smoke",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "phase10-v3-smoke",
        command=[
            "rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/certification-phase10-v3-smoke.toml --out-dir target/bench/phase10-v3-smoke --seed 7 --repetitions 1 --warmup 0",
        ],
        config_path="crates/bench/bench/certification-phase10-v3-smoke.toml",
        source_files=[
            "crates/bench/src/certify.rs",
            "crates/bench/src/workload.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/config.rs",
            "crates/bench/src/engine/sqlite.rs",
        ],
    ),
    SuiteSpec(
        suite_id="phase10-v3-cert",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "xbabe1" / "phase10-v3-cert",
        command=[
            "rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/certification-phase10-v3.toml --out-dir target/bench/xbabe1/phase10-v3-cert --seed 7 --repetitions 5 --warmup 1",
        ],
        config_path="crates/bench/bench/certification-phase10-v3.toml",
        source_files=[
            "crates/bench/src/certify.rs",
            "crates/bench/src/workload.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/config.rs",
            "crates/bench/src/engine/sqlite.rs",
        ],
    ),
    SuiteSpec(
        suite_id="phase10-v3-compare",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "xbabe1" / "phase10-v3-compare",
        command=[
            "rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/certification-phase10-v3-compare.toml --out-dir target/bench/xbabe1/phase10-v3-compare --seed 7 --repetitions 5 --warmup 1",
        ],
        config_path="crates/bench/bench/certification-phase10-v3-compare.toml",
        source_files=[
            "crates/bench/src/certify.rs",
            "crates/bench/src/workload.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/engine/sqlite.rs",
            "crates/bench/src/config.rs",
        ],
    ),
    SuiteSpec(
        suite_id="phase10-v3-stress",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "xbabe1" / "phase10-v3-stress",
        command=[
            "rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/certification-phase10-v3-stress.toml --out-dir target/bench/xbabe1/phase10-v3-stress --seed 7 --repetitions 5 --warmup 1",
        ],
        config_path="crates/bench/bench/certification-phase10-v3-stress.toml",
        source_files=[
            "crates/bench/src/certify.rs",
            "crates/bench/src/workload.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/config.rs",
            "crates/bench/src/engine/sqlite.rs",
        ],
    ),
    SuiteSpec(
        suite_id="phase11-oltp-gap",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "phase11-oltp-gap",
        command=[
            "rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/phase11-oltp-gap.toml --out-dir target/bench/phase11-oltp-gap --seed 7 --repetitions 3 --warmup 1",
        ],
        config_path="crates/bench/bench/phase11-oltp-gap.toml",
        source_files=[
            "crates/bench/src/certify.rs",
            "crates/bench/src/workload.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/engine/sqlite.rs",
            "crates/bench/src/config.rs",
            "crates/sql/src/exec/index_access.rs",
            "crates/sql/src/exec/index_batch.rs",
            "crates/sql/src/exec/select_top.rs",
        ],
    ),
    SuiteSpec(
        suite_id="dick-head-choas-smoke",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "dick-head-choas-smoke",
        command=[
            "rtk cargo test -p redlinedb-bench --test lane_bh chaos_suite_workloads_smoke --locked",
            "rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/dick-head-choas.toml --out-dir target/bench/dick-head-choas-smoke --seed 7 --repetitions 1 --warmup 0",
        ],
        config_path="crates/bench/bench/dick-head-choas.toml",
        source_files=[
            "crates/bench/src/chaos.rs",
            "crates/bench/src/certify.rs",
            "crates/bench/src/workload.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/engine/sqlite.rs",
            "crates/bench/src/config.rs",
            "crates/bench/tests/lane_bh.rs",
        ],
    ),
    SuiteSpec(
        suite_id="dick-head-choas-bounded-20260430-113740",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "xbabe1" / "dick-head-choas-bounded-20260430-113740",
        command=[
            "./scripts/bench/dick_head_choas_xbabe1.sh bounded",
        ],
        config_path="crates/bench/bench/dick-head-choas-bounded.toml",
        source_files=[
            "crates/bench/src/chaos.rs",
            "crates/bench/src/certify.rs",
            "crates/bench/src/workload.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/engine/sqlite.rs",
            "crates/bench/src/config.rs",
            "crates/bench/tests/lane_bh.rs",
            "scripts/bench/dick_head_choas_xbabe1.sh",
        ],
    ),
    SuiteSpec(
        suite_id="dick-head-choas-extreme-20260430-120155",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "xbabe1" / "dick-head-choas-extreme-20260430-120155",
        command=[
            "./scripts/bench/dick_head_choas_xbabe1.sh extreme",
        ],
        config_path="crates/bench/bench/dick-head-choas-extreme.toml",
        source_files=[
            "crates/bench/src/chaos.rs",
            "crates/bench/src/certify.rs",
            "crates/bench/src/workload.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/engine/sqlite.rs",
            "crates/bench/src/config.rs",
            "crates/bench/tests/lane_bh.rs",
            "scripts/bench/dick_head_choas_xbabe1.sh",
        ],
    ),
    SuiteSpec(
        suite_id="connection-limit-256-20260430-095514",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "xbabe1" / "connection-limit-256-20260430-095514",
        command=[
            "rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/connection-limit-256.toml --out-dir target/bench/xbabe1/connection-limit-256 --seed 7 --repetitions 3 --warmup 1",
        ],
        config_path="crates/bench/bench/connection-limit-256.toml",
        source_files=[
            "crates/bench/src/workload.rs",
            "crates/bench/src/certify.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/engine/sqlite.rs",
            "crates/bench/src/config.rs",
            "crates/bench/tests/lane_bh.rs",
        ],
    ),
    SuiteSpec(
        suite_id="connection-fixed-high-20260430-101216",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "xbabe1" / "connection-fixed-high-20260430-101216",
        command=[
            "rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/connection-fixed-high.toml --out-dir target/bench/xbabe1/connection-fixed-high --seed 7 --repetitions 3 --warmup 1",
        ],
        config_path="crates/bench/bench/connection-fixed-high.toml",
        source_files=[
            "crates/bench/src/workload.rs",
            "crates/bench/src/certify.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/engine/sqlite.rs",
            "crates/bench/src/config.rs",
            "crates/bench/tests/lane_bh.rs",
        ],
    ),
    SuiteSpec(
        suite_id="queue-mixed-highload-20260430-100133",
        kind="certify",
        input_path=DEFAULT_INPUT_ROOT / "xbabe1" / "queue-mixed-highload-20260430-100133",
        command=[
            "rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/queue-mixed-highload.toml --out-dir target/bench/xbabe1/queue-mixed-highload --seed 7 --repetitions 3 --warmup 1",
        ],
        config_path="crates/bench/bench/queue-mixed-highload.toml",
        source_files=[
            "crates/bench/src/workload.rs",
            "crates/bench/src/certify.rs",
            "crates/bench/src/report.rs",
            "crates/bench/src/engine/mod.rs",
            "crates/bench/src/engine/redline.rs",
            "crates/bench/src/engine/sqlite.rs",
            "crates/bench/src/config.rs",
            "crates/bench/tests/lane_bh.rs",
        ],
    ),
    SuiteSpec(
        suite_id="phase9-recovery-matrix",
        kind="json",
        input_path=DEFAULT_INPUT_ROOT / "recovery-matrix.json",
        command=[
            "rtk cargo run -p redlinedb-bench -- recover-matrix --config crates/bench/bench/recovery-matrix.toml --out target/bench/recovery-matrix.json --seed 7",
        ],
        config_path="crates/bench/bench/recovery-matrix.toml",
        source_files=[
            "crates/bench/src/recover.rs",
            "crates/bench/src/config.rs",
            "crates/kernel/src/engine/mod.rs",
            "crates/kernel/src/engine/page_heap.rs",
            "crates/kernel/src/wal/manager.rs",
            "crates/kernel/src/wal/payload.rs",
        ],
    ),
    SuiteSpec(
        suite_id="phase9-failpoint-matrix",
        kind="json",
        input_path=DEFAULT_INPUT_ROOT / "failpoint-matrix.json",
        command=[
            "rtk cargo run -p redlinedb-bench -- failpoint-matrix --config crates/bench/bench/failpoint-matrix.toml --out target/bench/failpoint-matrix.json --seed 7",
        ],
        config_path="crates/bench/bench/failpoint-matrix.toml",
        source_files=[
            "crates/bench/src/failpoint_matrix.rs",
            "crates/bench/src/config.rs",
            "crates/kernel/src/engine/mod.rs",
            "crates/kernel/src/engine/page_heap.rs",
            "crates/kernel/src/index/mod.rs",
            "crates/kernel/src/index/mutate.rs",
            "crates/kernel/src/wal/manager.rs",
            "crates/kernel/src/wal/payload.rs",
            "crates/kernel/src/telemetry/mod.rs",
        ],
    ),
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--version-root",
        default=str(DEFAULT_VERSION_ROOT),
        help="root directory for versioned JSON exports",
    )
    parser.add_argument(
        "--input-root",
        default=str(DEFAULT_INPUT_ROOT),
        help="benchmark artifact root to export from",
    )
    parser.add_argument(
        "--version-number",
        default=os.environ.get("VERSION_NUMBER"),
        help="version folder name; defaults to the current git SHA",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="fail if any official suite is missing from the input root",
    )
    return parser.parse_args()


def repo_git_sha() -> str:
    return subprocess.check_output(
        ["git", "-C", str(REPO_ROOT), "rev-parse", "HEAD"],
        text=True,
    ).strip()


def repo_dirty() -> bool:
    status = subprocess.check_output(
        ["git", "-C", str(REPO_ROOT), "status", "--porcelain"],
        text=True,
    )
    return bool(status.strip())


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def read_json(path: Path) -> dict[str, Any]:
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    records = []
    with open(path, "r", encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if line:
                records.append(json.loads(line))
    return records


def maybe_read_json(path: Path) -> dict[str, Any] | None:
    return read_json(path) if path.exists() else None


def median(values: Iterable[float]) -> float:
    values = list(values)
    return statistics.median(values) if values else float("nan")


def load_manifest(path: Path) -> dict[str, Any] | None:
    return maybe_read_json(path / "manifest.json")


def load_report(path: Path) -> dict[str, Any] | None:
    return maybe_read_json(path / "report.json")


def source_file_hashes(paths: Iterable[str]) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    for rel in sorted(set(paths)):
        path = REPO_ROOT / rel
        if path.exists():
            out.append({"path": rel, "sha256": sha256_file(path)})
        else:
            out.append({"path": rel, "sha256": None})
    return out


def infer_certify_sources(report: dict[str, Any], config_path: str | None) -> list[str]:
    workloads = [str(run.get("workload", "")) for run in report.get("runs", [])]
    source_files = {
        "crates/bench/src/certify.rs",
        "crates/bench/src/config.rs",
        "crates/bench/src/engine/mod.rs",
        "crates/bench/src/engine/redline.rs",
        "crates/bench/src/engine/sqlite.rs",
        "crates/bench/src/report.rs",
        "crates/bench/src/workload.rs",
    }
    if any(workload.startswith("chaos-") for workload in workloads):
        source_files.add("crates/bench/src/chaos.rs")
        source_files.add("crates/bench/tests/lane_bh.rs")
        source_files.add("scripts/bench/dick_head_choas_xbabe1.sh")
    if any(workload.startswith("secondary-index-") or workload.startswith("covered-range-") for workload in workloads):
        source_files.update(
            {
                "crates/sql/src/exec/index_access.rs",
                "crates/sql/src/exec/index_batch.rs",
                "crates/sql/src/exec/select_top.rs",
            }
        )
    if any(workload in {"hot-counter-update", "writers-disjoint"} for workload in workloads):
        source_files.add("crates/sql/tests/phase11_w1_cde.rs")
    if config_path:
        source_files.add(config_path)
    return sorted(source_files)


def infer_json_sources(spec: SuiteSpec) -> list[str]:
    source_files = set(spec.source_files)
    if spec.config_path:
        source_files.add(spec.config_path)
    source_files.add("scripts/bench/export_benchmark_results.py")
    return sorted(source_files)


def build_certify_export(spec: SuiteSpec, input_dir: Path) -> dict[str, Any]:
    manifest = load_manifest(input_dir)
    report = load_report(input_dir)
    if manifest is None or report is None:
        return {}
    runs = report.get("runs", []) if report else []
    runs_jsonl = input_dir / "runs.jsonl"
    ratio_csv = input_dir / "ratio.csv"
    summary_csv = input_dir / "summary.csv"
    export = {
        "schema_version": 1,
        "suite_id": spec.suite_id,
        "suite_kind": spec.kind,
        "input_dir": str(input_dir),
        "command": spec.command,
        "config_path": spec.config_path,
        "config_hash": sha256_file(REPO_ROOT / spec.config_path) if spec.config_path else None,
        "git_sha": manifest.get("git_sha") if manifest else repo_git_sha(),
        "git_dirty": manifest.get("git_dirty") if manifest else repo_dirty(),
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "source_files": source_file_hashes(infer_certify_sources(report or {}, spec.config_path)),
        "artifacts": {
            "manifest_json": {
                "path": str(input_dir / "manifest.json"),
                "sha256": sha256_file(input_dir / "manifest.json") if (input_dir / "manifest.json").exists() else None,
            },
            "report_json": {
                "path": str(input_dir / "report.json"),
                "sha256": sha256_file(input_dir / "report.json") if (input_dir / "report.json").exists() else None,
            },
            "runs_jsonl": {
                "path": str(runs_jsonl),
                "sha256": sha256_file(runs_jsonl) if runs_jsonl.exists() else None,
            },
            "ratio_csv": {
                "path": str(ratio_csv),
                "sha256": sha256_file(ratio_csv) if ratio_csv.exists() else None,
            },
            "summary_csv": {
                "path": str(summary_csv),
                "sha256": sha256_file(summary_csv) if summary_csv.exists() else None,
            },
        },
        "report": report,
        "runs": runs,
    }
    return export


def build_json_export(spec: SuiteSpec, input_path: Path) -> dict[str, Any]:
    payload = read_json(input_path)
    export = {
        "schema_version": 1,
        "suite_id": spec.suite_id,
        "suite_kind": spec.kind,
        "input_path": str(input_path),
        "command": spec.command,
        "config_path": spec.config_path,
        "config_hash": sha256_file(REPO_ROOT / spec.config_path) if spec.config_path else None,
        "git_sha": repo_git_sha(),
        "git_dirty": repo_dirty(),
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "source_files": source_file_hashes(infer_json_sources(spec)),
        "artifacts": {
            "json": {
                "path": str(input_path),
                "sha256": sha256_file(input_path),
            }
        },
        "payload": payload,
    }
    return export


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")


def export_suite(spec: SuiteSpec, version_dir: Path, input_root: Path) -> dict[str, Any] | None:
    input_path = (input_root / spec.input_path.relative_to(DEFAULT_INPUT_ROOT)) if spec.input_path.is_relative_to(DEFAULT_INPUT_ROOT) else spec.input_path
    if not input_path.exists():
        return None
    if spec.kind == "certify":
        export = build_certify_export(spec, input_path)
        if not export:
            return None
    else:
        export = build_json_export(spec, input_path)
    suite_path = version_dir / "suites" / f"{spec.suite_id}.json"
    write_json(suite_path, export)
    export["export_path"] = str(suite_path)
    return export


def refresh_index(version_dir: Path, version_number: str, exports: list[dict[str, Any]]) -> Path:
    index = {
        "schema_version": 1,
        "version_number": version_number,
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "git_sha": repo_git_sha(),
        "git_dirty": repo_dirty(),
        "suite_count": len(exports),
        "suites": [
            {
                "suite_id": export["suite_id"],
                "suite_kind": export["suite_kind"],
                "export_path": export["export_path"],
                "config_path": export.get("config_path"),
                "command": export.get("command"),
                "source_files": [item["path"] for item in export["source_files"]],
            }
            for export in exports
        ],
    }
    index_path = version_dir / "index.json"
    write_json(index_path, index)
    return index_path


def main() -> int:
    args = parse_args()
    input_root = Path(args.input_root)
    version_root = Path(args.version_root)
    version_number = args.version_number or repo_git_sha()
    version_dir = version_root / version_number
    if version_dir.exists():
        shutil.rmtree(version_dir)
    version_dir.mkdir(parents=True, exist_ok=True)

    exports: list[dict[str, Any]] = []
    missing: list[str] = []
    for spec in SUITES:
        export = export_suite(spec, version_dir, input_root)
        if export is None:
            missing.append(spec.suite_id)
            continue
        exports.append(export)

    index_path = refresh_index(version_dir, version_number, exports)
    summary = {
        "version_dir": str(version_dir),
        "index_path": str(index_path),
        "exported_suites": [export["suite_id"] for export in exports],
        "missing_suites": missing,
    }
    print(json.dumps(summary, indent=2, sort_keys=True))
    if args.strict and missing:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
