#!/usr/bin/env python3
from __future__ import annotations

import argparse
import fnmatch
import json
import subprocess
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore[no-redef]


def load(path: Path) -> dict[str, Any]:
    with path.open("rb") as fh:
        return tomllib.load(fh)


def git_files(source_root: Path, source_sha: str) -> list[str]:
    result = subprocess.run(
        ["git", "-C", str(source_root), "ls-tree", "-r", "--name-only", source_sha],
        check=True,
        text=True,
        capture_output=True,
    )
    return [line for line in result.stdout.splitlines() if line.strip()]


def matches(path: str, pattern: str) -> bool:
    if fnmatch.fnmatch(path, pattern):
        return True
    if pattern.endswith("/**") and path.startswith(pattern[:-3]):
        base = pattern[:-3]
        return path == base or path.startswith(base + "/")
    return path == pattern


def covered(path: str, patterns: list[str]) -> bool:
    for pattern in patterns:
        if matches(path, pattern):
            return True
    return False


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", default="repos.manifest.toml")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    manifest_path = Path(args.manifest)
    data = load(manifest_path)
    source_root = Path(str(data["source_root"]))
    source_sha = str(data["source_sha"])

    ownership: dict[str, list[str]] = {}
    retired: list[str] = [str(item) for item in data.get("retired_paths", [])]
    shared: list[str] = [str(item) for item in data.get("shared_source_paths", [])]
    for repo in data.get("repo", []):
        name = str(repo.get("name", "<unknown>"))
        for pattern in repo.get("source_paths", []):
            ownership.setdefault(str(pattern), []).append(name)

    files = git_files(source_root, source_sha)
    missing: list[str] = []
    duplicates: dict[str, list[str]] = {}
    retired_count = 0
    shared_count = 0
    owned_count = 0
    for path in files:
        owners: list[str] = []
        for pattern, names in ownership.items():
            if matches(path, pattern):
                owners.extend(names)
        is_retired = covered(path, retired)
        is_shared = covered(path, shared)
        classes = int(bool(owners)) + int(is_retired) + int(is_shared)
        if classes == 0:
            missing.append(path)
        elif len(owners) > 1 or classes > 1:
            duplicates[path] = owners + (["retired"] if is_retired else []) + (["shared"] if is_shared else [])
        elif owners:
            owned_count += 1
        elif is_retired:
            retired_count += 1
        elif is_shared:
            shared_count += 1
    report = {
        "schema_version": "jain.split.source-coverage/v1",
        "source_root": str(source_root),
        "source_sha": source_sha,
        "tracked_files": len(files),
        "source_patterns": len(ownership),
        "retired_patterns": len(retired),
        "shared_patterns": len(shared),
        "owned_count": owned_count,
        "retired_count": retired_count,
        "shared_count": shared_count,
        "missing_count": len(missing),
        "duplicate_count": len(duplicates),
        "missing": missing,
        "duplicates": duplicates,
        "status": "pass" if not missing and not duplicates else "fail",
    }
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    elif missing or duplicates:
        print(
            f"source coverage failed: {len(missing)} missing, {len(duplicates)} duplicate assignments"
        )
        for path in missing[:100]:
            print(f"missing: {path}")
        for path, owners in list(duplicates.items())[:100]:
            print(f"duplicate: {path} -> {', '.join(owners)}")
    else:
        print(
            f"source coverage pass: {len(files)} tracked files; {owned_count} owned, {retired_count} retired, {shared_count} shared"
        )
    return 0 if not missing else 1


if __name__ == "__main__":
    raise SystemExit(main())
