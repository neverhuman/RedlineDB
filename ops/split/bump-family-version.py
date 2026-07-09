#!/usr/bin/env python3
"""Bump Jain split-family tags, workspace versions, and lock pins.

This script is manifest-driven and intentionally does not assume a fixed repo
set. Run with --update-lock-shas after release commits exist.
"""
from __future__ import annotations

import argparse
import re
import subprocess
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "repos.manifest.toml"


def load_manifest(path: Path) -> dict:
    with path.open("rb") as fh:
        return tomllib.load(fh)


def rewrite(path: Path, update) -> bool:
    if not path.exists():
        return False
    original = path.read_text(encoding="utf-8")
    changed = update(original)
    if changed != original:
        path.write_text(changed, encoding="utf-8")
        print(f"changed: {path}")
        return True
    return False


def repo_paths(data: dict) -> list[Path]:
    return [Path(str(repo["path"])) for repo in data.get("repo", [])]


def bump_manifest(path: Path, old: str, new: str) -> None:
    old_suffix = f"v{old}-split.0"
    new_suffix = f"v{new}-split.0"

    def update(text: str) -> str:
        text = text.replace(f'dependency_tag_suffix = "{old_suffix}"', f'dependency_tag_suffix = "{new_suffix}"')
        text = re.sub(
            rf'(current_tag = "jain(?:-[a-z0-9-]+)?)-{re.escape(old_suffix)}"',
            rf'\1-{new_suffix}"',
            text,
        )
        return text

    rewrite(path, update)


def bump_repo_files(data: dict, old: str, new: str) -> None:
    old_tag = f"v{old}-split.0"
    new_tag = f"v{new}-split.0"
    # A dep pin may sit on ANY split revision of the old version, not just `.0`: a repo whose
    # feat-core pin was refreshed out-of-band carries e.g. `v7.0.1-split.1`. Matching the literal
    # `.0` would leave that pin behind on the previous version's core — the exact drift this
    # family bump exists to end. Match every revision.
    old_tag_any_rev = re.compile(rf"v{re.escape(old)}-split\.\d+")
    for root in repo_paths(data):
        rewrite(root / "VERSION", lambda _: f"{root.name}-{new_tag}\n")
        rewrite(root / "Cargo.toml", lambda text: text.replace(f'version = "{old}"', f'version = "{new}"', 1))
        for toml in root.rglob("Cargo.toml"):
            if "target" in toml.parts:
                continue
            rewrite(toml, lambda text: old_tag_any_rev.sub(new_tag, text))
        changelog = root / "CHANGELOG.md"
        if changelog.exists() and new_tag not in changelog.read_text(encoding="utf-8"):
            entry = f"## {root.name}-{new_tag}\n\n- Split-family release pin refresh.\n\n"
            rewrite(changelog, lambda text, e=entry: text.replace("\n", "\n\n" + e, 1) if text.startswith("# ") else e + text)


def update_lock_shas(data: dict) -> None:
    locks = [ROOT / "jain" / "family.lock", ROOT / "jain-deploy" / "jain-split.lock.toml"]
    shas: dict[str, str] = {}
    for repo in data.get("repo", []):
        root = Path(str(repo["path"]))
        if (root / ".git").exists():
            shas[str(repo["name"])] = subprocess.check_output(
                ["git", "-C", str(root), "rev-parse", "HEAD"],
                text=True,
            ).strip()
    for lock in locks:
        def update(text: str) -> str:
            for name, sha in shas.items():
                text = re.sub(
                    rf'(\[\[repo\]\]\nrepo = "{re.escape(name)}"\n(?:.*\n)*?commit = ")[^"]+(")',
                    rf"\g<1>{sha}\2",
                    text,
                    count=1,
                )
            return text

        rewrite(lock, update)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", default=str(MANIFEST))
    parser.add_argument("--old", default="7.0.1")
    parser.add_argument("--new", required=False)
    parser.add_argument("--update-lock-shas", action="store_true")
    args = parser.parse_args()

    data = load_manifest(Path(args.manifest))
    if args.update_lock_shas:
        update_lock_shas(data)
        return 0
    if not args.new:
        raise SystemExit("--new is required unless --update-lock-shas is used")
    bump_manifest(Path(args.manifest), args.old, args.new)
    for manifest in (ROOT / "jain" / "repos.manifest.toml", ROOT / "jain-deploy" / "repos.manifest.toml"):
        bump_manifest(manifest, args.old, args.new)
    bump_repo_files(data, args.old, args.new)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
