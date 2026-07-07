#!/usr/bin/env python3
"""Map Jain monorepo drift into split repos from repos.manifest.toml.

By default this writes a report only. Use --apply to copy changed files from the
source worktree into the owning split repo for paths covered by source_paths.
"""
from __future__ import annotations

import argparse
import fnmatch
import json
import shutil
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "repos.manifest.toml"


def load(path: Path) -> dict[str, Any]:
    with path.open("rb") as fh:
        return tomllib.load(fh)


def run(args: list[str], cwd: Path) -> str:
    return subprocess.check_output(args, cwd=cwd, text=True).strip()


def matches(path: str, pattern: str) -> bool:
    if fnmatch.fnmatch(path, pattern):
        return True
    if pattern.endswith("/**"):
        base = pattern[:-3].rstrip("/")
        return path == base or path.startswith(base + "/")
    return path == pattern


def changed_paths(source_root: Path, base_ref: str) -> set[str]:
    paths: set[str] = set()
    diff = run(["git", "diff", "--name-only", base_ref, "HEAD"], source_root)
    paths.update(line for line in diff.splitlines() if line)
    status = run(["git", "status", "--porcelain"], source_root)
    for line in status.splitlines():
        if len(line) > 3:
            paths.add(line[3:].strip())
    return paths


def owner_for(path: str, repos: list[dict[str, Any]]) -> list[dict[str, Any]]:
    owners = []
    for repo in repos:
        if any(matches(path, str(pattern)) for pattern in repo.get("source_paths", [])):
            owners.append(repo)
    return owners


def copy_path(source_root: Path, repo_root: Path, rel: str) -> str:
    src = source_root / rel
    dest = repo_root / rel
    if not src.exists():
        if dest.exists():
            if dest.is_dir():
                shutil.rmtree(dest)
            else:
                dest.unlink()
        return "deleted"
    if src.is_dir():
        if dest.exists():
            shutil.rmtree(dest)
        shutil.copytree(src, dest)
    else:
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dest)
    return "copied"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", default=str(MANIFEST))
    parser.add_argument("--base-ref", default=None)
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--json", default=None)
    args = parser.parse_args()

    data = load(Path(args.manifest))
    source_root = Path(str(data["source_root"]))
    base_ref = args.base_ref or str(data["source_sha"])
    repos = list(data.get("repo", []))
    report = {
        "schema_version": "jain.split.reconcile/v1",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "source_root": str(source_root),
        "base_ref": base_ref,
        "apply": args.apply,
        "items": [],
    }
    for rel in sorted(changed_paths(source_root, base_ref)):
        owners = owner_for(rel, repos)
        if not owners:
            report["items"].append({"path": rel, "disposition": "unmapped"})
            continue
        if len(owners) > 1:
            report["items"].append({"path": rel, "disposition": "ambiguous", "repos": [r["name"] for r in owners]})
            continue
        repo = owners[0]
        disposition = "planned"
        if args.apply:
            disposition = copy_path(source_root, Path(str(repo["path"])), rel)
        report["items"].append({"path": rel, "repo": repo["name"], "disposition": disposition})

    out = Path(args.json) if args.json else ROOT / "target" / "reconcile-report.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"wrote {out}")
    unmapped = [item for item in report["items"] if item["disposition"] in {"unmapped", "ambiguous"}]
    return 1 if unmapped else 0


if __name__ == "__main__":
    raise SystemExit(main())
