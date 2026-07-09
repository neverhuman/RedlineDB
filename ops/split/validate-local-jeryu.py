#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore[no-redef]


LOCAL_JERYU_PREFIX = "http://127.0.0.1:8787/git/jeryu/"
FORBIDDEN_SOURCE_MARKERS = (
    "github.com/neverhuman",
    "git@github.com:neverhuman",
    "jain-portal-preview",
)
OPERATIONAL_TEXT_MARKERS = FORBIDDEN_SOURCE_MARKERS + ("bare-mirrors",)
CONFUSING_WORKSPACE_MARKERS = ("jeryu-split", "/home/ubuntu/jeryu-split")
SKIP_DIRS = {".git", "target", ".venv", "node_modules", ".stage", "vendor"}
ROOT = Path(__file__).resolve().parents[2]


def load_manifest(path: Path) -> list[dict[str, Any]]:
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    repos = data.get("repo", [])
    if not isinstance(repos, list) or not repos:
        raise SystemExit(f"{path} has no [[repo]] entries")
    return [repo for repo in repos if isinstance(repo, dict)]


def expected_remote(repo: dict[str, Any]) -> str:
    return f"{LOCAL_JERYU_PREFIX}{repo['name']}.git"


def iter_files(root: Path, names: set[str] | None = None) -> list[Path]:
    paths: list[Path] = []
    if not root.exists():
        return paths
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        if any(part in SKIP_DIRS for part in path.relative_to(root).parts):
            continue
        if names is None or path.name in names:
            paths.append(path)
    return paths


def git_remote_urls(root: Path) -> dict[str, list[str]]:
    names = subprocess.check_output(["git", "-C", str(root), "remote"], text=True).splitlines()
    remotes: dict[str, list[str]] = {}
    for name in names:
        urls = subprocess.check_output(["git", "-C", str(root), "remote", "get-url", "--all", name], text=True).splitlines()
        remotes[name] = urls
    return remotes


def workspace_root(repos: list[dict[str, Any]]) -> Path:
    if not repos:
        return ROOT.parent
    return Path(str(repos[0]["path"])).parent


def workspace_git_roots(repos: list[dict[str, Any]]) -> list[Path]:
    split_root = workspace_root(repos)
    roots = [path.parent for path in split_root.glob("*/.git")]
    if (ROOT / ".git").exists() and ROOT not in roots:
        roots.append(ROOT)
    return sorted(set(roots))


def check_remotes(repos: list[dict[str, Any]], errors: list[str]) -> None:
    for repo in repos:
        root = Path(str(repo["path"]))
        if not (root / ".git").exists():
            errors.append(f"{repo['name']}: missing git checkout at {root}")
            continue
        expected = expected_remote(repo)
        try:
            remotes = git_remote_urls(root)
        except subprocess.CalledProcessError as exc:
            errors.append(f"{repo['name']}: cannot read remotes: {exc}")
            continue
        if remotes.get("origin") != [expected]:
            errors.append(f"{repo['name']}: origin must be exactly {expected}")
        for remote, urls in remotes.items():
            for url in urls:
                if url != expected:
                    errors.append(f"{repo['name']}: remote {remote} points outside local Jeryu: {url}")
                for marker in FORBIDDEN_SOURCE_MARKERS:
                    if marker in url:
                        errors.append(f"{repo['name']}: remote {remote} contains forbidden marker {marker}: {url}")


def check_workspace_remotes(repos: list[dict[str, Any]], errors: list[str]) -> None:
    for root in workspace_git_roots(repos):
        name = root.name
        try:
            remotes = git_remote_urls(root)
        except subprocess.CalledProcessError as exc:
            errors.append(f"{name}: cannot read remotes: {exc}")
            continue
        if set(remotes) != {"origin"}:
            errors.append(f"{name}: expected only origin remote, found {', '.join(sorted(remotes))}")
        for remote, urls in remotes.items():
            for url in urls:
                if not url.startswith(LOCAL_JERYU_PREFIX):
                    errors.append(f"{name}: remote {remote} must use local Jeryu: {url}")
                for marker in FORBIDDEN_SOURCE_MARKERS:
                    if marker in url:
                        errors.append(f"{name}: remote {remote} contains forbidden marker {marker}: {url}")


def check_cargo_sources(repos: list[dict[str, Any]], errors: list[str]) -> None:
    cargo_names = {"Cargo.toml", "Cargo.lock"}
    local_source = re.compile(r"(?:git\+)?http://127\.0\.0\.1:8787/git/jeryu/jain[-a-z]*\.git")
    for repo in repos:
        root = Path(str(repo["path"]))
        for path in iter_files(root, cargo_names):
            text = path.read_text(encoding="utf-8", errors="replace")
            rel = path.relative_to(root)
            for marker in FORBIDDEN_SOURCE_MARKERS + ("bare-mirrors",):
                if marker in text:
                    errors.append(f"{repo['name']}:{rel}: Cargo source contains forbidden marker {marker}")
            for line_no, line in enumerate(text.splitlines(), 1):
                if "git" not in line or "jain" not in line:
                    continue
                if "http://127.0.0.1:8787/git/jeryu/" in line:
                    if not local_source.search(line):
                        errors.append(f"{repo['name']}:{rel}:{line_no}: malformed local Jeryu source")
                    continue
                if "git =" in line or line.startswith("source = \"git+"):
                    errors.append(f"{repo['name']}:{rel}:{line_no}: internal git source is not local Jeryu")


def operational_paths(root: Path) -> list[Path]:
    wanted: list[Path] = []
    exact = {
        "AGENTS.md",
        "README.md",
        "Justfile",
        "docs/local-jeryu-forge-agent-workflow.md",
        "ops/AGENTS.md",
        "ops/dev/local-patches.example.toml",
        "scripts/clone-family.sh",
        "scripts/ci-local.sh",
    }
    for rel in exact:
        path = root / rel
        if path.exists():
            wanted.append(path)
    for pattern in ("ops/ci/*.sh", ".github/workflows/*.yml"):
        wanted.extend(path for path in root.glob(pattern) if path.is_file())
    return sorted(set(wanted))


def line_allowed(path: Path, line: str) -> bool:
    rel = path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else path.as_posix()
    lowered = line.lower()
    if "do not" in lowered or "never" in lowered or "not canonical" in lowered or "ci cache" in lowered:
        return True
    if "bare-mirrors" in line and "where" in lowered:
        return True
    if "public release" in lowered or "archive" in lowered or "archival" in lowered:
        return True
    if rel == "ops/ci/split-host-ci.sh" and "insteadOf = https://github.com/neverhuman/" in line:
        return True
    if rel == "ops/split/materialize.py":
        public_release = (
            "releases/latest/download" in line
            or "releases/download" in line
            or "--certificate-identity-regexp" in line
            or "github_remote" in line
        )
        ci_cache = "insteadOf = {repo.github_remote}" in line or "bare-mirrors" in line
        return public_release or ci_cache
    return False


def check_operational_text(repos: list[dict[str, Any]], errors: list[str]) -> None:
    paths: list[Path] = []
    paths.extend(operational_paths(ROOT))
    paths.extend([ROOT / "ops" / "split" / "materialize.py", ROOT / "ops" / "ci" / "split-host-ci.sh"])
    for repo in repos:
        paths.extend(operational_paths(Path(str(repo["path"]))))
    for root in workspace_git_roots(repos):
        paths.extend(operational_paths(root))
    for path in sorted(set(paths)):
        if not path.exists():
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for line_no, line in enumerate(text.splitlines(), 1):
            markers = OPERATIONAL_TEXT_MARKERS + CONFUSING_WORKSPACE_MARKERS
            if not any(marker in line for marker in markers):
                continue
            if line_allowed(path, line):
                continue
            errors.append(f"{path}:{line_no}: operational source must use local Jeryu: {line.strip()}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", default=str(ROOT / "repos.manifest.toml"))
    parser.add_argument("--skip-remotes", action="store_true")
    args = parser.parse_args()
    repos = load_manifest(Path(args.manifest))
    errors: list[str] = []
    if not args.skip_remotes:
        check_remotes(repos, errors)
        check_workspace_remotes(repos, errors)
    check_cargo_sources(repos, errors)
    check_operational_text(repos, errors)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"local Jeryu policy ok: {len(repos)} repos")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
