#!/usr/bin/env bash
# Validate and normalize the official redline-testing evidence bundle.
#
# The official gate writes its raw artifacts under target/redline-testing/.
# This helper verifies the JSON contract, recomputes hashes for every declared
# output file, enforces the verified runner SHA, and emits a processed summary
# at target/redline-testing/official-evidence.processed.json.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

root="${1:-target/redline-testing}"
official_evidence="$root/official-evidence.json"
provenance="$root/redline-testing-provenance.env"
processed="$root/official-evidence.processed.json"

if [ ! -s "$official_evidence" ]; then
    printf 'redline-testing evidence processor: missing official evidence %s\n' "$official_evidence" >&2
    exit 1
fi

if [ ! -s "$provenance" ]; then
    printf 'redline-testing evidence processor: missing provenance %s\n' "$provenance" >&2
    exit 1
fi

set -a
# shellcheck source=/dev/null
. "$provenance"
set +a

python3 - "$repo_root" "$root" "$official_evidence" "$processed" <<'PY'
import hashlib
import json
import os
import pathlib
import sys
import time
from collections.abc import Mapping

repo_root = pathlib.Path(sys.argv[1])
root = pathlib.Path(sys.argv[2])
official_path = pathlib.Path(sys.argv[3])
processed_path = pathlib.Path(sys.argv[4])

EXPECTED_SCHEMA = "redline-testing-official-evidence-v1"
PROCESSED_SCHEMA = "redline-testing-official-evidence-processed-v1"
REQUIRED_TOP_LEVEL_FIELDS = {
    "schema_version",
    "runner",
    "target",
    "sqlite",
    "suites",
    "status",
    "command_line",
    "generated_at_unix_ms",
    "output_file_hashes",
}
REQUIRED_SUITE_NAMES = ("sqlite_parity", "memory", "beyond_sqlite")
ROOT_REQUIRED_PATHS = (
    "all.jsonl",
    "all-manifest.json",
    "summary.json",
    "manifest.json",
    "provenance.json",
    "memory-summary.json",
    "memory-manifest.json",
    "memory-provenance.json",
    "beyond-sqlite-summary.json",
    "beyond-sqlite-manifest.json",
    "beyond-sqlite-provenance.json",
)


def fail(message: str) -> None:
    raise SystemExit(f"redline-testing evidence processor: {message}")


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def normalize_path(value: str) -> str:
    return value.strip().replace("\\", "/").removeprefix("./")


def normalize_hash(value: object) -> str | None:
    if isinstance(value, str):
        candidate = value.strip().lower()
        if candidate.startswith("sha256:"):
            candidate = candidate[len("sha256:") :]
        if len(candidate) == 64 and all(ch in "0123456789abcdef" for ch in candidate):
            return candidate
    return None


def dig(value: object, *path: str) -> object | None:
    current = value
    for segment in path:
        if not isinstance(current, Mapping):
            return None
        current = current.get(segment)
    return current


def first_string(value: object, *paths: tuple[str, ...]) -> str | None:
    for path in paths:
        candidate = dig(value, *path)
        if isinstance(candidate, str) and candidate.strip():
            return candidate.strip()
    return None


def first_hash(value: object, *paths: tuple[str, ...]) -> str | None:
    for path in paths:
        candidate = normalize_hash(dig(value, *path))
        if candidate:
            return candidate
    return None


def normalize_hash_map(value: object) -> dict[str, str]:
    hashes: dict[str, str] = {}
    if isinstance(value, Mapping):
        for key, item in value.items():
            if isinstance(item, Mapping):
                path = first_string(item, ("path",), ("file",), ("name",))
                hash_value = first_hash(
                    item,
                    ("sha256",),
                    ("hash",),
                    ("digest",),
                    ("value",),
                )
                if path and hash_value:
                    hashes[normalize_path(path)] = hash_value
                    continue
                if hash_value:
                    hashes[normalize_path(str(key))] = hash_value
                    continue
            hash_value = normalize_hash(item)
            if hash_value:
                hashes[normalize_path(str(key))] = hash_value
    elif isinstance(value, list):
        for item in value:
            if not isinstance(item, Mapping):
                continue
            path = first_string(item, ("path",), ("file",), ("name",))
            hash_value = first_hash(
                item,
                ("sha256",),
                ("hash",),
                ("digest",),
                ("value",),
            )
            if path and hash_value:
                hashes[normalize_path(path)] = hash_value
    return hashes


def lookup_hash(hashes: dict[str, str], *candidates: str) -> str | None:
    for candidate in candidates:
        normalized = normalize_path(candidate)
        if normalized in hashes:
            return hashes[normalized]
    return None


def require_hash(hashes: dict[str, str], *candidates: str) -> str:
    value = lookup_hash(hashes, *candidates)
    if not value:
        fail(f"official evidence does not declare a hash for any of: {', '.join(candidates)}")
    return value


def suite_map(suites: object) -> dict[str, dict[str, object]]:
    if isinstance(suites, Mapping):
        mapped: dict[str, dict[str, object]] = {}
        for name, entry in suites.items():
            if not isinstance(entry, Mapping):
                fail(f"suite {name!r} is not an object")
            mapped[normalize_path(str(name))] = dict(entry)
        return mapped
    if isinstance(suites, list):
        mapped = {}
        for entry in suites:
            if not isinstance(entry, Mapping):
                fail("suite entry is not an object")
            name = first_string(entry, ("name",), ("suite",))
            if not name:
                fail("suite entry is missing a name")
            mapped[normalize_path(name)] = dict(entry)
        return mapped
    fail("suites must be an object or array")
    return {}


def suite_int(entry: Mapping[str, object], *keys: str) -> int:
    for key in keys:
        value = entry.get(key)
        if isinstance(value, bool):
            continue
        if isinstance(value, int):
            return value
        if isinstance(value, str) and value.isdigit():
            return int(value)
    fail(f"suite entry missing integer field: {', '.join(keys)}")
    raise AssertionError


def suite_path(entry: Mapping[str, object], *keys: str) -> str:
    for key in keys:
        value = entry.get(key)
        if isinstance(value, str) and value.strip():
            return normalize_path(value)
    fail(f"suite entry missing path field: {', '.join(keys)}")
    raise AssertionError


def parse_status(status: object) -> str:
    if isinstance(status, str) and status.strip():
        return status.strip().lower()
    fail("status is missing or not a string")
    raise AssertionError


def runner_sha(evidence: Mapping[str, object]) -> str:
    runner = evidence.get("runner")
    if not isinstance(runner, Mapping):
        fail("runner field is missing or not an object")
    for candidate in (
        ("binary_sha256",),
        ("sha256",),
        ("release_binary_sha256",),
        ("release_artifact", "binary_sha256"),
        ("release_artifact", "bin_sha256"),
        ("binary", "sha256"),
    ):
        value = first_hash(runner, candidate)
        if value:
            return value
    fail("runner object does not expose a binary SHA-256")
    raise AssertionError


official = json.loads(official_path.read_text(encoding="utf-8"))
missing_top_level = sorted(REQUIRED_TOP_LEVEL_FIELDS - set(official))
if missing_top_level:
    fail(f"official evidence missing top-level field(s): {', '.join(missing_top_level)}")

if official["schema_version"] != EXPECTED_SCHEMA:
    fail(
        f"official evidence schema_version {official['schema_version']!r} "
        f"!= {EXPECTED_SCHEMA!r}"
    )

status = parse_status(official["status"])
if status not in {"passed", "pass", "success", "succeeded", "ok"}:
    fail(f"official evidence status is not successful: {official['status']!r}")

output_hashes = normalize_hash_map(official["output_file_hashes"])
if not output_hashes:
    fail("official evidence output_file_hashes is empty")

expected_runner_sha = (
    os.environ.get("CI_REDLINE_TESTING_RELEASE_BINARY_SHA256")
    or os.environ.get("CI_REDLINE_TESTING_BIN_SHA256")
    or os.environ.get("CI_REDLINE_TESTING_EXPECTED_BINARY_SHA256")
)
if not expected_runner_sha:
    fail("verified redline-testing runner SHA-256 is unavailable from provenance")
expected_runner_sha = normalize_hash(expected_runner_sha)
if not expected_runner_sha:
    fail("verified redline-testing runner SHA-256 is not a valid SHA-256 digest")

observed_runner_sha = runner_sha(official)
if observed_runner_sha != expected_runner_sha:
    fail(
        "runner SHA-256 mismatch: "
        f"expected {expected_runner_sha}, got {observed_runner_sha}"
    )

required_paths = set(ROOT_REQUIRED_PATHS)
suite_entries = suite_map(official["suites"])
missing_suites = [name for name in REQUIRED_SUITE_NAMES if name not in suite_entries]
if missing_suites:
    fail(f"official evidence missing suite(s): {', '.join(missing_suites)}")

validated_suites: dict[str, dict[str, object]] = {}
for suite_name in REQUIRED_SUITE_NAMES:
    entry = suite_entries[suite_name]
    total = suite_int(entry, "total")
    passed = suite_int(entry, "passed")
    failed = suite_int(entry, "failed")
    skipped = suite_int(entry, "skipped")
    raw_path = suite_path(entry, "raw_path", "raw")
    summary_path = suite_path(entry, "summary_path", "summary")
    ranked_path = suite_path(entry, "ranked_path", "ranked")
    manifest_path = suite_path(entry, "manifest_path", "manifest")
    provenance_path = suite_path(entry, "provenance_path", "provenance")

    required_paths.update(
        {
            raw_path,
            summary_path,
            ranked_path,
            manifest_path,
            provenance_path,
        }
    )

    if failed != 0:
        fail(f"suite {suite_name} failed {failed} test(s)")
    if suite_name in {"sqlite_parity", "memory"}:
        if total != 1127 or passed != 1127 or skipped != 0:
            fail(
                f"suite {suite_name} expected 1127/1127 with zero skips, "
                f"got total={total} passed={passed} skipped={skipped}"
            )
    else:
        if passed + skipped != total:
            fail(
                f"suite {suite_name} has inconsistent totals: "
                f"passed={passed} skipped={skipped} total={total}"
            )

    validated_suites[suite_name] = {
        "total": total,
        "passed": passed,
        "failed": failed,
        "skipped": skipped,
        "raw_path": raw_path,
        "raw_sha256": None,
        "summary_path": summary_path,
        "summary_sha256": None,
        "ranked_path": ranked_path,
        "ranked_sha256": None,
        "manifest_path": manifest_path,
        "manifest_sha256": None,
        "provenance_path": provenance_path,
        "provenance_sha256": None,
    }

for relative_path in sorted(required_paths):
    file_path = root / relative_path
    if not file_path.is_file():
        fail(f"required evidence file is missing: {relative_path}")
    actual_sha256 = sha256_file(file_path)
    expected_sha256 = lookup_hash(
        output_hashes,
        relative_path,
        str(file_path),
        str((repo_root / file_path).resolve()),
        file_path.name,
    )
    if expected_sha256 is None:
        fail(f"official evidence does not declare a hash for {relative_path}")
    if actual_sha256 != expected_sha256:
        fail(
            f"sha256 mismatch for {relative_path}: "
            f"expected {expected_sha256}, got {actual_sha256}"
        )

for suite_name, entry in validated_suites.items():
    entry["raw_sha256"] = require_hash(
        output_hashes,
        entry["raw_path"],
        str(root / entry["raw_path"]),
        str((repo_root / root / entry["raw_path"]).resolve()),
        pathlib.Path(entry["raw_path"]).name,
    )
    entry["summary_sha256"] = require_hash(
        output_hashes,
        entry["summary_path"],
        str(root / entry["summary_path"]),
        str((repo_root / root / entry["summary_path"]).resolve()),
        pathlib.Path(entry["summary_path"]).name,
    )
    entry["manifest_sha256"] = require_hash(
        output_hashes,
        entry["manifest_path"],
        str(root / entry["manifest_path"]),
        str((repo_root / root / entry["manifest_path"]).resolve()),
        pathlib.Path(entry["manifest_path"]).name,
    )
    entry["ranked_sha256"] = require_hash(
        output_hashes,
        entry["ranked_path"],
        str(root / entry["ranked_path"]),
        str((repo_root / root / entry["ranked_path"]).resolve()),
        pathlib.Path(entry["ranked_path"]).name,
    )
    entry["provenance_sha256"] = require_hash(
        output_hashes,
        entry["provenance_path"],
        str(root / entry["provenance_path"]),
        str((repo_root / root / entry["provenance_path"]).resolve()),
        pathlib.Path(entry["provenance_path"]).name,
    )

processed = {
    "schema_version": PROCESSED_SCHEMA,
    "source_path": str(official_path),
    "source_sha256": sha256_file(official_path),
    "validated_at_unix_ms": int(time.time() * 1000),
    "generated_at_unix_ms": official["generated_at_unix_ms"],
    "runner_expected_binary_sha256": expected_runner_sha,
    "runner_observed_binary_sha256": observed_runner_sha,
    "status": "passed",
    "command_line": official["command_line"],
    "target": official["target"],
    "sqlite": official["sqlite"],
    "suite_summaries": validated_suites,
    "output_file_hashes": output_hashes,
    "official_evidence": official,
}

processed_path.write_text(json.dumps(processed, indent=2, sort_keys=True) + "\n", encoding="utf-8")
print(f"redline-testing evidence processed: {processed_path}")
PY
