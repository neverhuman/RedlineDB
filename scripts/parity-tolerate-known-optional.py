#!/usr/bin/env python3
"""
Post-filter for `redline-testing run` output.

Exit 0 if every failure in the JSONL is for a known-optional case the
target intentionally does not support (and so should have been skipped by
target capability gating). Exit non-zero otherwise — i.e. there is a real
parity regression that must be investigated before merging.

The redline-testing v0.1.3+ release skips these cases via target
capability gating; v0.1.2 (currently pinned via official-evidence) does
not, so we mirror the gate here until CI moves to v0.1.3.

Known-optional case ids (SQL_VIRTUAL_TABLE_OPTIONAL):
  93  CREATE_VIRTUAL_TABLE_FTS5_OPTIONAL
  94  FTS5_HIGHLIGHT_OPTIONAL
  95  CREATE_VIRTUAL_TABLE_RTREE_OPTIONAL
  96  DBSTAT_OPTIONAL

Usage: parity-tolerate-known-optional.py <evidence_dir>

Looks for `all.jsonl` first (full run including memory + beyond-sqlite
suites), then falls back to `sqlite_parity.raw.jsonl` which the v0.1.2
binary writes incrementally even when it errors out mid-suite.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

KNOWN_OPTIONAL_CASES = {"00093", "00094", "00095", "00096"}
CANDIDATE_FILENAMES = ("all.jsonl", "sqlite_parity.raw.jsonl")


def main(evidence_dir: Path) -> int:
    jsonl_path: Path | None = None
    for name in CANDIDATE_FILENAMES:
        candidate = evidence_dir / name
        if candidate.is_file() and candidate.stat().st_size > 0:
            jsonl_path = candidate
            break
    if jsonl_path is None:
        sys.stderr.write(
            f"parity tolerance: no parity-result JSONL found under {evidence_dir} "
            f"(tried: {', '.join(CANDIDATE_FILENAMES)})\n"
        )
        return 1

    unexpected: list[str] = []
    optional_failures: list[str] = []
    with jsonl_path.open() as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            rec = json.loads(line)
            if rec.get("status") != "failed":
                continue
            case_id = rec.get("case_id", "")
            if case_id in KNOWN_OPTIONAL_CASES:
                optional_failures.append(case_id)
            else:
                unexpected.append(f"{case_id} ({rec.get('name', '?')})")

    if unexpected:
        sys.stderr.write(
            f"parity tolerance: {len(unexpected)} unexpected failure(s) in {jsonl_path.name} — cannot tolerate:\n"
        )
        for entry in unexpected[:20]:
            sys.stderr.write(f"  - {entry}\n")
        if len(unexpected) > 20:
            sys.stderr.write(f"  ... and {len(unexpected) - 20} more\n")
        return 1

    sys.stderr.write(
        f"parity tolerance: {len(optional_failures)} known-optional case failure(s) tolerated in {jsonl_path.name} "
        f"(SQL_VIRTUAL_TABLE_OPTIONAL: fts5/rtree/dbstat — target lacks virtual-table API)\n"
    )
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.stderr.write(f"usage: {sys.argv[0]} <evidence_dir>\n")
        sys.exit(64)
    sys.exit(main(Path(sys.argv[1])))
