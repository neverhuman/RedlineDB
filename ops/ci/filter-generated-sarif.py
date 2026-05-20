#!/usr/bin/env python3
"""Remove generated-zone findings from jankurai SARIF before code scanning upload."""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
import tomllib
from pathlib import Path
from typing import Any


def load_generated_zones(path: Path) -> list[str]:
    with path.open("rb") as handle:
        data = tomllib.load(handle)
    zones = []
    for zone in data.get("zone", []):
        zone_path = str(zone.get("path", "")).strip()
        if not zone_path or zone_path == ".jankurai/generated-zones.toml":
            continue
        zones.append(normalize_uri(zone_path))
    return zones


def normalize_uri(uri: str) -> str:
    while uri.startswith("./"):
        uri = uri[2:]
    return uri.replace("\\", "/")


def is_generated(uri: str, zones: list[str]) -> bool:
    uri = normalize_uri(uri)
    for zone in zones:
        if zone.endswith("/"):
            if uri.startswith(zone):
                return True
        elif uri == zone:
            return True
    return False


def result_is_generated(result: dict[str, Any], zones: list[str]) -> bool:
    for location in result.get("locations", []):
        artifact = (
            location.get("physicalLocation", {})
            .get("artifactLocation", {})
            .get("uri")
        )
        if artifact and is_generated(str(artifact), zones):
            return True
    return False


def filter_sarif(sarif: dict[str, Any], zones: list[str]) -> int:
    removed = 0
    for run in sarif.get("runs", []):
        results = run.get("results", [])
        kept = []
        for result in results:
            if result_is_generated(result, zones):
                removed += 1
            else:
                kept.append(result)
        run["results"] = kept
    return removed


def write_json_atomic(path: Path, data: dict[str, Any]) -> None:
    with tempfile.NamedTemporaryFile("w", delete=False, dir=path.parent) as handle:
        json.dump(data, handle, indent=2)
        handle.write("\n")
        tmp_path = Path(handle.name)
    tmp_path.replace(path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("sarif", type=Path)
    parser.add_argument(
        "--generated-zones",
        type=Path,
        default=Path(".jankurai/generated-zones.toml"),
    )
    args = parser.parse_args()

    if not args.sarif.exists():
        print(f"SARIF file not found: {args.sarif}", file=sys.stderr)
        return 1
    if not args.generated_zones.exists():
        print(f"generated zones file not found: {args.generated_zones}", file=sys.stderr)
        return 1

    zones = load_generated_zones(args.generated_zones)
    sarif = json.loads(args.sarif.read_text())
    before = sum(len(run.get("results", [])) for run in sarif.get("runs", []))
    removed = filter_sarif(sarif, zones)
    after = sum(len(run.get("results", [])) for run in sarif.get("runs", []))
    write_json_atomic(args.sarif, sarif)
    print(
        f"filtered generated-zone SARIF findings: before={before} removed={removed} after={after}",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
