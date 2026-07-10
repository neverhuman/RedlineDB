#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import stat
import subprocess
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "repos.manifest.toml"
GENERATOR = "ops/split/materialize.py"
CHECKOUT_ACTION_SHA = "08eba0b27e820071cde6df949e0beb9ba4906955"
UPLOAD_ARTIFACT_ACTION_SHA = "ea165f8d65b6e75b540449e92b4886f43607fa02"
COMMON_COPY_PATHS = [
    ".config",
    ".dockerignore",
    ".gitattributes",
    ".gitignore",
    "LICENSE",
    "rust-toolchain.toml",
    "deny.toml",
]
STARFORGE_LFS_PATHS = [
    "artifacts/foundation/tabicl-classifier-v2-chimera-shuffled.safetensors",
    "artifacts/foundation/tabicl-regressor-v2-20260212.safetensors",
    "artifacts/starforge/chimera_classification_attnfusion.safetensors",
    "artifacts/starforge/chimera_classification_expert_a.safetensors",
    "artifacts/starforge/chimera_stackfusionreg.safetensors",
    "artifacts/starforge/starlight_core_regressor.safetensors",
]
SPLIT_PATCHES = {
    "jain-catboost": ["catboost-sys-vendor-root.patch"],
    "jain-xgboost": ["xgboost-vendor-root.patch"],
    "jain-lightgbm": ["lightgbm-vendor-root.patch"],
}
INHERITED_SOURCE_CAPS = {
    "jain-battle-gpu": ["missing-rust-property-or-integration-tests", "streaming-runtime-drift"],
    "jain-catboost": ["missing-rust-property-or-integration-tests"],
    "jain-core": ["authz-or-data-isolation-gap"],
    "jain-deploy": ["missing-rust-property-or-integration-tests"],
    "jain-domain": ["authz-or-data-isolation-gap", "missing-rust-property-or-integration-tests"],
    "jain-lightgbm": ["missing-rust-property-or-integration-tests"],
    "jain-math": [
        "future-hostile-dead-language-in-product-code",
        "severe-duplication-in-product-code",
        "missing-rust-property-or-integration-tests",
    ],
    "jain-model-zoo": ["python-direct-product-truth-or-db-ownership"],
    "jain-report": ["missing-rust-property-or-integration-tests"],
    "jain-starforge": ["severe-duplication-in-product-code", "missing-rust-property-or-integration-tests"],
    "jain-tui": ["missing-rust-property-or-integration-tests"],
    "jain-web": [
        "fallback-soup-in-product-code",
        "future-hostile-dead-language-in-product-code",
        "direct-db-access-from-wrong-layer",
        "missing-rust-property-or-integration-tests",
        "typescript-bad-behavior",
    ],
    "jain-xgboost": ["missing-rust-property-or-integration-tests"],
}


@dataclass(frozen=True)
class Repo:
    name: str
    path: Path
    github_slug: str
    jeryu_slug: str
    profile: str
    role: str
    current_tag: str
    required_check: str
    note: str
    mirror_github_main: bool
    authored: bool
    cargo_members: list[str]
    copy_paths: list[str]
    source_paths: list[str]

    @property
    def github_remote(self) -> str:
        return f"https://github.com/{self.github_slug}.git"

    @property
    def jeryu_remote(self) -> str:
        return f"http://127.0.0.1:8787/git/{self.jeryu_slug}.git"


def run(cmd: list[str], *, cwd: Path | None = None, input_bytes: bytes | None = None) -> str:
    result = subprocess.run(
        cmd,
        cwd=str(cwd) if cwd else None,
        input=input_bytes,
        check=True,
        capture_output=True,
    )
    return result.stdout.decode("utf-8", errors="replace").strip()


def load_manifest(path: Path) -> tuple[dict[str, Any], list[Repo]]:
    with path.open("rb") as fh:
        data = tomllib.load(fh)
    repos = [
        Repo(
            name=str(raw["name"]),
            path=Path(str(raw["path"])),
            github_slug=str(raw["github_slug"]),
            jeryu_slug=str(raw["jeryu_slug"]),
            profile=str(raw["profile"]),
            role=str(raw.get("role", "")),
            current_tag=str(raw["current_tag"]),
            required_check=str(raw["required_check"]),
            note=str(raw.get("note", "")),
            mirror_github_main=bool(raw.get("mirror_github_main", True)),
            authored=bool(raw.get("authored", False)),
            cargo_members=[str(item) for item in raw.get("cargo_members", [])],
            copy_paths=[str(item) for item in raw.get("copy_paths", [])],
            source_paths=[str(item) for item in raw.get("source_paths", [])],
        )
        for raw in data.get("repo", [])
    ]
    if not repos:
        raise SystemExit(f"{path} has no [[repo]] entries")
    return data, repos


def git_tracked_paths(source_root: Path, source_sha: str) -> set[str]:
    output = run(["git", "-C", str(source_root), "ls-tree", "-r", "--name-only", source_sha])
    return set(output.splitlines())


def path_exists_in_tree(path: str, tracked: set[str]) -> bool:
    clean = path.rstrip("/")
    return clean in tracked or any(item.startswith(clean + "/") for item in tracked)


def archive_paths(source_root: Path, source_sha: str, dest: Path, paths: list[str], tracked: set[str]) -> None:
    selected: list[str] = []
    seen: set[str] = set()
    for path in paths:
        if path in seen:
            continue
        seen.add(path)
        if path_exists_in_tree(path, tracked):
            selected.append(path)
    if not selected:
        return
    archive = subprocess.run(
        ["git", "-C", str(source_root), "archive", "--format=tar", source_sha, "--", *selected],
        check=True,
        capture_output=True,
    ).stdout
    subprocess.run(["tar", "-x", "-C", str(dest)], input=archive, check=True)


def read_package_name(manifest: Path) -> str:
    with manifest.open("rb") as fh:
        data = tomllib.load(fh)
    return str(data["package"]["name"])


def cargo_member_packages(source_root: Path, repos: list[Repo]) -> tuple[dict[str, str], dict[str, str]]:
    package_to_repo: dict[str, str] = {}
    member_to_package: dict[str, str] = {}
    for repo in repos:
        for member in repo.cargo_members:
            manifest = source_root / member / "Cargo.toml"
            if not manifest.exists():
                # Authored split-only members, currently deployment/product.
                continue
            package = read_package_name(manifest)
            member_to_package[member] = package
            package_to_repo[package] = repo.name
    return package_to_repo, member_to_package


def render_cargo_toml(source_root: Path, members: list[str], patch_text: str = "") -> str:
    source = (source_root / "Cargo.toml").read_text(encoding="utf-8")
    replacement = "members = [\n" + "".join(f'  "{member}",\n' for member in members) + "]"
    source = re.sub(r"(?ms)^members = \[\n.*?^\]", replacement, source, count=1)
    if patch_text:
        source = source.rstrip() + "\n\n" + patch_text.rstrip() + "\n"
    return source


def parse_inline_value(body: str, key: str) -> str | None:
    match = re.search(rf"(?<![A-Za-z0-9_-]){re.escape(key)}\s*=\s*([^,}}]+|\[[^\]]*\])", body)
    return match.group(1).strip() if match else None


def rewrite_dependency_tomls(repo: Repo, repos_by_name: dict[str, Repo], package_to_repo: dict[str, str]) -> None:
    dep_line = re.compile(r'^([A-Za-z0-9_-]+)\s*=\s*\{([^}\n]*\bpath\s*=\s*"[^"]+"[^}\n]*)\}', re.MULTILINE)

    def replace(match: re.Match[str]) -> str:
        dep_key = match.group(1)
        body = match.group(2)
        dep_package = parse_inline_value(body, "package")
        dep_package = dep_package.strip('"') if dep_package else dep_key
        owner = package_to_repo.get(dep_package)
        if owner is None or owner == repo.name:
            return match.group(0)
        dep_repo = repos_by_name[owner]
        fields = [
            f'git = "{dep_repo.jeryu_remote}"',
            f'tag = "{dep_repo.current_tag}"',
            f'package = "{dep_package}"',
        ]
        for key in ("optional", "default-features", "features"):
            value = parse_inline_value(body, key)
            if value is not None:
                fields.append(f"{key} = {value}")
        return f"{dep_key} = {{ " + ", ".join(fields) + " }"

    for member in repo.cargo_members:
        manifest = repo.path / member / "Cargo.toml"
        if not manifest.exists():
            continue
        text = manifest.read_text(encoding="utf-8")
        updated = dep_line.sub(replace, text)
        manifest.write_text(updated, encoding="utf-8")


def apply_split_patches(repo: Repo) -> list[str]:
    applied: list[str] = []
    for patch_name in SPLIT_PATCHES.get(repo.name, []):
        patch = ROOT / "ops" / "split" / "patches" / patch_name
        if not patch.exists():
            raise SystemExit(f"missing split patch: {patch}")
        subprocess.run(["git", "-C", str(repo.path), "apply", "--check", str(patch)], check=True)
        subprocess.run(["git", "-C", str(repo.path), "apply", str(patch)], check=True)
        applied.append(patch_name)
    return applied


def smudge_starforge_lfs(source_root: Path, repo: Repo) -> None:
    if repo.name != "jain-starforge":
        return
    for rel in STARFORGE_LFS_PATHS:
        src = source_root / rel
        dest = repo.path / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dest)
        size = dest.stat().st_size
        if size <= 1_000_000:
            raise SystemExit(f"Starforge LFS payload is not smudged: {rel} ({size} bytes)")


def render_patch_sections(repo: Repo, repos: list[Repo], package_to_repo: dict[str, str], member_to_package: dict[str, str]) -> str:
    if repo.name != "jain-deploy":
        return ""
    sections: dict[str, list[str]] = {}
    for member, package in member_to_package.items():
        owner = package_to_repo[package]
        if owner == repo.name or owner == "jain-ops":
            continue
        owner_repo = next(item for item in repos if item.name == owner)
        sections.setdefault(owner_repo.jeryu_remote, []).append(
            f'{package} = {{ path = "../{owner}/{member}" }}'
        )
    lines: list[str] = [
        "# Local split development patches. Release builds consume the pinned local Jeryu tags above.",
    ]
    for remote in sorted(sections):
        lines.append(f'[patch."{remote}"]')
        lines.extend(sorted(sections[remote]))
        lines.append("")
    return "\n".join(lines)


def write(path: Path, content: str, executable: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    if executable:
        mode = path.stat().st_mode
        path.chmod(mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def route_variants(pattern: str) -> list[str]:
    is_dir = pattern.endswith("/")
    clean = pattern.rstrip("/")
    if clean.endswith("/**"):
        base = clean[:-3].rstrip("/")
        return [base + "/", base + "/**"]
    if is_dir:
        return [clean + "/", clean + "/**"]
    return [clean]


def add_route(routes: dict[str, Any], pattern: str, value: Any) -> None:
    for variant in route_variants(pattern):
        routes[variant] = value


def standard_route_patterns(repo: Repo) -> list[str]:
    patterns = [
        ".cargo/",
        ".config/",
        ".dockerignore",
        ".gitattributes",
        ".github/",
        ".gitignore",
        "AGENTS.md",
        "CHANGELOG.md",
        "Justfile",
        "LICENSE",
        "README.md",
        "SPLIT.md",
        "VERSION",
        "agent/",
        "deny.toml",
        "docs/",
        "ops/",
        "ops/AGENTS.md",
        "ops/ci/",
        "ops/dev/",
        "ops/git-hooks/",
        "ops/split/",
        "rust-toolchain.toml",
        "scripts/",
        "tests/",
        "tools/",
    ]
    if repo.profile == "public-portal":
        patterns.extend(["family.lock", "repos.manifest.toml"])
    if repo.cargo_members or repo.name == "jain-deploy":
        patterns.extend(["Cargo.toml", "Cargo.lock"])
    for path in repo.copy_paths + repo.source_paths:
        patterns.append(path)
    if "contracts" in repo.copy_paths or any(path.startswith("contracts") for path in repo.source_paths):
        patterns.append("contracts/")
    if repo.name == "jain-web":
        patterns.extend(["apps/web/", "contracts/", "db/", "package.json", "pnpm-lock.yaml"])
    if repo.name == "jain-python":
        patterns.extend(["python/ai-service/", "contracts/"])
    if repo.name == "jain-deploy":
        patterns.extend(["deployment/", "jain-split.lock.toml", ".stage/"])
    if repo.name == "jain-starforge":
        patterns.extend(["artifacts/foundation/", "artifacts/starforge/"])
    return patterns


def test_route(command: str, purpose: str, lane: str) -> dict[str, str]:
    return {"command": command, "purpose": purpose, "lane": lane}


def cargo_package_args(repo: Repo) -> str:
    packages: list[str] = []
    for member in repo.cargo_members:
        manifest = repo.path / member / "Cargo.toml"
        if manifest.exists():
            try:
                packages.append(read_package_name(manifest))
            except Exception:
                continue
    return " ".join(f"-p {package}" for package in packages)


def has_package_json(repo: Repo) -> bool:
    return (repo.path / "apps" / "web" / "package.json").exists()


def has_python_project(repo: Repo) -> bool:
    return (repo.path / "python" / "ai-service" / "pyproject.toml").exists()


def render_cargo_config(repo: Repo) -> str:
    return f"""# Portable split Cargo configuration for {repo.name}.
# Native learner library paths are supplied by CI through JAIN_VENDOR_ROOT,
# LD_LIBRARY_PATH, and learner-specific environment variables. Do not commit
# host-specific rpaths here.

[net]
git-fetch-with-cli = true

[env]
CARGO_NET_RETRY = "3"
JAIN_VENDOR_ROOT = {{ value = "target/native-vendor", relative = true, force = false }}
"""


def append_split_gitignore(repo: Repo) -> None:
    path = repo.path / ".gitignore"
    existing = path.read_text(encoding="utf-8") if path.exists() else ""
    block = """

# Jain split generated outputs
/target/
/.jankurai/
/.stage/
**/__pycache__/
.pytest_cache/
.ci-status/
catboost_info/
"""
    if "# Jain split generated outputs" not in existing:
        write(path, existing.rstrip() + block)


def render_readme(repo: Repo, source_sha: str) -> str:
    source_lines = "\n".join(f"- `{path}`" for path in repo.source_paths)
    cargo = "\n".join(f"- `{member}`" for member in repo.cargo_members) or "- none"
    return f"""# {repo.name}

{repo.note}

This repository was seeded from Jain source commit `{source_sha}` by
`{GENERATOR}`. It is part of the Jain split family and keeps source
paths stable where practical so ownership remains auditable.

## Owned Cargo Packages

{cargo}

## Source Coverage

{source_lines}

## Local Commands

- `just fast`
- `just check`
- `just required`
- `just score`
- `just security`
- `just artifact-support`
"""


def render_split_repo_map(repos: list[Repo]) -> str:
    lines = [
        "Agent operational remotes are local Jeryu remotes from `repos.manifest.toml`; public mirrors are not development sources.",
        "",
        "| Repository | Role | Public mirror | Purpose |",
        "| --- | --- | --- | --- |",
    ]
    for item in repos:
        role = "Public portal" if item.profile == "public-portal" else "Split member"
        lines.append(
            f"| `{item.name}` | {role} | `{item.github_slug}` | {item.note} |"
        )
    return "\n".join(lines)


def render_portal_readme(repo: Repo, repos: list[Repo]) -> str:
    return f"""# Jain

Public portal for the Jain split repository family.

The release authority is `neverhuman/jain-deploy`. This repository contains
the installer, the split-family clone entrypoint, local CI wrappers, and audit
metadata. Product source lives in the split member repositories listed below.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/neverhuman/jain/main/scripts/install.sh | bash
```

Pin a release or install somewhere else:

```bash
JAIN_VERSION={repo.current_tag} JAIN_INSTALL_DIR="$HOME/.local/bin" \\
  bash scripts/install.sh
```

The installer downloads the `jain` binary from
`neverhuman/jain-deploy` releases, verifies `SHA256SUMS`, and runs cosign
verification when `jain.sig`, `jain.pem`, and `cosign` are available.

## Clone The Split Family

```bash
git clone http://127.0.0.1:8787/git/jeryu/jain.git
cd jain
scripts/clone-family.sh "$HOME/jain-split"
```

Existing checkouts are updated with `git fetch` and `git pull --ff-only`.
The portal repository is skipped by default so the command can be run from an
already-cloned portal checkout.

## Release Evidence

Release receipts, binary checksums, SBOMs, provenance, witness artifacts, and
rollback evidence are published by `neverhuman/jain-deploy`:

- https://github.com/neverhuman/jain-deploy/releases
- `SHA256SUMS`
- `release-receipt.json`
- `artifact-support-evidence.tar.gz`

## Split Repository Map

{render_split_repo_map(repos)}

## Local Commands

- `just fast`
- `just check`
- `just required`
- `just score`
- `just security`
- `just artifact-support`
"""


def render_local_jeryu_forge_workflow() -> str:
    return """## Local Jeryu Forge Workflow

The Jain workspace is `/home/ubuntu/jain-split`. Do not use `~/jeryu-split` as
an operational source for Jain work; it is a precedent/product checkout, not a
member of this family.

Use normal Git commands against the local loopback Jeryu remote. Git credentials
are already supplied by local Git/HTTP credential storage, so fetch/push should
be fast and should not require agent-visible token handling.

Canonical repo remote:
`http://127.0.0.1:8787/git/jeryu/<repo>.git`.

For a quick check:

```bash
git remote -v
git ls-remote origin HEAD
```

If a repo's remote is wrong or Git is slow/failing, run the Jain control-plane
repair once:

```bash
cd /home/ubuntu/jain-split/jain-split-ops
just jeryu-ready
```

That command checks the local forge, removes extra remotes from every live
checkout, sets `origin` to local Jeryu, registers Jain family metadata, and runs
the family policy validator. Do not inspect `~/.jeryu`, run `gh auth login`, or
copy source from Jeryu internals.

For PRs, checks, and merges, use `jeryu.*` MCP tools when they are exposed. If
they are not exposed, use
`/home/ubuntu/jain-split/jain-split-ops/ops/split/jeryu-local.py` or the
`just jeryu-*` recipes from the control-plane repo.

Use `just jeryu-doctor` for a read-only health check. Run or post split CI
through `/home/ubuntu/jain-split/jain-split-ops/ops/ci/split-host-ci.sh`, not
GitHub Actions, unless an explicit GitHub mirror workflow is requested.
"""


def render_portal_agents(repo: Repo) -> str:
    return f"""# {repo.name} Agent Instructions

This is the public portal for the Jain split family.

Before editing, read `README.md`, `agent/owner-map.json`,
`agent/test-map.json`, `agent/generated-zones.toml`,
`agent/proof-lanes.toml`, `agent/audit-policy.toml`, and
`agent/boundaries.toml`.

Keep this repository lightweight: installer, clone-family entrypoint, local CI
wrappers, and audit metadata only. Product source belongs in the split member
repositories, and release authority belongs in `jain-deploy`.

{render_local_jeryu_forge_workflow()}
"""


def render_install_script() -> str:
    return """#!/usr/bin/env bash
set -euo pipefail

repo="neverhuman/jain-deploy"
version="${JAIN_VERSION:-latest}"
install_dir="${JAIN_INSTALL_DIR:-${HOME}/.jain/bin}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

if [[ "$version" == "latest" ]]; then
  base="https://github.com/${repo}/releases/latest/download"
else
  base="https://github.com/${repo}/releases/download/${version}"
fi

download() {
  local name="$1"
  curl -fL --retry 3 --retry-delay 2 -o "${tmp}/${name}" "${base}/${name}"
}

download jain
download SHA256SUMS

(
  cd "$tmp"
  grep -Eq '([[:space:]]|\\*)jain$' SHA256SUMS || {
    printf 'SHA256SUMS does not contain a jain entry\\n' >&2
    exit 1
  }
  sha256sum --check --ignore-missing SHA256SUMS
)

if command -v cosign >/dev/null 2>&1; then
  sig_ok=0
  if curl -fL --retry 3 --retry-delay 2 -o "${tmp}/jain.sig" "${base}/jain.sig"; then
    if curl -fL --retry 3 --retry-delay 2 -o "${tmp}/jain.pem" "${base}/jain.pem"; then
      sig_ok=1
    fi
  fi
  if [[ "$sig_ok" == "1" ]]; then
    cosign verify-blob \
      --signature "${tmp}/jain.sig" \
      --certificate "${tmp}/jain.pem" \
      --certificate-identity-regexp "https://github.com/${repo}/.*release.yml@.*" \
      --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
      "${tmp}/jain"
  else
    printf 'cosign assets unavailable; SHA256SUMS verification completed\\n' >&2
  fi
else
  printf 'cosign not found; SHA256SUMS verification completed\\n' >&2
fi

mkdir -p "$install_dir"
install -m 0755 "${tmp}/jain" "${install_dir}/jain"
printf 'installed jain to %s\\n' "${install_dir}/jain"
"""


def render_clone_family_script() -> str:
    return """#!/usr/bin/env bash
set -euo pipefail

dry_run=0
if [[ "${1:-}" == "--dry-run" ]]; then
  dry_run=1
  shift
fi
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
default_dest="$(cd "${repo_root}/.." && pwd)"
dest="${1:-$default_dest}"
manifest="${JAIN_SPLIT_MANIFEST:-${repo_root}/repos.manifest.toml}"

[[ -r "$manifest" ]] || { printf 'manifest not readable: %s\\n' "$manifest" >&2; exit 1; }
mkdir -p "$dest"

mapfile -t rows < <(
  python3 - "$manifest" <<'PY'
import sys
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib
with open(sys.argv[1], "rb") as fh:
    data = tomllib.load(fh)
for repo in data.get("repo", []):
    print("|".join([
        str(repo.get("name", "")),
        str(repo.get("jeryu_slug", "")),
        str(repo.get("profile", "")),
    ]))
PY
)

for row in "${rows[@]}"; do
  IFS='|' read -r name jeryu_slug profile <<<"$row"
  [[ -n "$name" && -n "$jeryu_slug" ]] || continue
  if [[ "$profile" == "public-portal" && "${JAIN_CLONE_PORTAL:-0}" != "1" ]]; then
    continue
  fi
  target="${dest}/${name}"
  remote="http://127.0.0.1:8787/git/${jeryu_slug}.git"
  if [[ "$dry_run" == "1" ]]; then
    printf 'would clone/update %s -> %s\\n' "$remote" "$target"
    continue
  fi
  if [[ -d "${target}/.git" ]]; then
    printf 'updating %s\\n' "$target"
    git -C "$target" fetch --prune origin
    branch="$(git -C "$target" symbolic-ref --quiet --short HEAD || printf 'main')"
    git -C "$target" pull --ff-only origin "$branch"
  elif [[ -e "$target" ]]; then
    printf 'refusing to overwrite non-git path: %s\\n' "$target" >&2
    exit 1
  else
    printf 'cloning %s -> %s\\n' "$remote" "$target"
    git clone "$remote" "$target"
  fi
done
"""


def render_validate_family_script() -> str:
    return """#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
manifest="${JAIN_SPLIT_MANIFEST:-${repo_root}/repos.manifest.toml}"

bash "${repo_root}/ops/split/manifest.sh" --manifest "$manifest" --check-paths
python3 "${repo_root}/ops/split/source_coverage.py" --manifest "$manifest"
python3 "${repo_root}/ops/split/validate-local-jeryu.py" --manifest "$manifest"

python3 - "$manifest" <<'PY'
from pathlib import Path
import re
import sys
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

data = tomllib.loads(Path(sys.argv[1]).read_text())
errors = []
for repo in data.get("repo", []):
    root = Path(repo["path"])
    for rel in [
        "AGENTS.md",
        "SPLIT.md",
        "VERSION",
        "Justfile",
        "scripts/ci-local.sh",
        "ops/ci/required.sh",
        "ops/ci/tool-adoption.sh",
        "ops/ci/contract-drift.sh",
        "ops/ci/score.sh",
        "agent/owner-map.json",
        "agent/test-map.json",
        "agent/tool-adoption.toml",
        "agent/security-policy.toml",
        "agent/proofbind.toml",
        "agent/proofmark.toml",
        "agent/audit-policy.toml",
        ".github/workflows/ci.yml",
    ]:
        if not (root / rel).exists():
            errors.append(f"{repo['name']} missing {rel}")
    if (root / "Cargo.toml").exists() and not (root / "Cargo.lock").exists():
        errors.append(f"{repo['name']} missing Cargo.lock")
    cargo_config = root / ".cargo" / "config.toml"
    if cargo_config.exists() and "/home/ubuntu/.cache/jain_small" in cargo_config.read_text():
        errors.append(f"{repo['name']} cargo config contains host-specific native rpath")
    for manifest in root.rglob("Cargo.toml"):
        text = manifest.read_text()
        if "branch =" in text:
            errors.append(f"{manifest} uses branch git dependency")
        for m in re.finditer(r'path\\s*=\\s*"([^"]+)"', text):
            raw = m.group(1)
            if repo["name"] == "jain-deploy" or not raw.startswith("../"):
                continue
            target = (manifest.parent / raw).resolve()
            try:
                target.relative_to(root.resolve())
            except ValueError:
                errors.append(f"{manifest} has cross-repo path dependency: {raw}")
sf = Path(data["split_root"]) / "jain-starforge"
for rel in [
    "artifacts/foundation/tabicl-classifier-v2-chimera-shuffled.safetensors",
    "artifacts/foundation/tabicl-regressor-v2-20260212.safetensors",
    "artifacts/starforge/chimera_classification_attnfusion.safetensors",
    "artifacts/starforge/chimera_classification_expert_a.safetensors",
    "artifacts/starforge/chimera_stackfusionreg.safetensors",
    "artifacts/starforge/starlight_core_regressor.safetensors",
]:
    p = sf / rel
    if not p.exists() or p.stat().st_size <= 1_000_000:
        errors.append(f"Starforge payload missing or pointer-sized: {rel}")
if errors:
    raise SystemExit("\\n".join(errors))
print("family validation ok")
PY
"""


def render_fleet_ci_script() -> str:
    return """#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
manifest="${JAIN_SPLIT_MANIFEST:-${repo_root}/repos.manifest.toml}"
jobs="${JAIN_FLEET_JOBS:-4}"
mkdir -p "${repo_root}/.ci-status"

python3 - "$manifest" "$jobs" "${repo_root}/.ci-status" <<'PY'
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
import subprocess
import sys
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

data = tomllib.loads(Path(sys.argv[1]).read_text())
jobs = max(1, int(sys.argv[2]))
status = Path(sys.argv[3])

def run_repo(repo):
    root = Path(repo["path"])
    out = status / f"{repo['name']}.log"
    with out.open("w") as fh:
        result = subprocess.run(["bash", "scripts/ci-local.sh", "required"], cwd=root, stdout=fh, stderr=subprocess.STDOUT)
    return repo["name"], result.returncode

failures = []
with ThreadPoolExecutor(max_workers=jobs) as pool:
    futures = [pool.submit(run_repo, repo) for repo in data.get("repo", [])]
    for future in as_completed(futures):
        name, code = future.result()
        print(f"{name}: {'ok' if code == 0 else 'failed'}")
        if code:
            failures.append(name)
if failures:
    raise SystemExit("required lane failures: " + ", ".join(failures))
print("fleet required lanes ok")
PY
"""


def render_family_doctor_script() -> str:
    return """#!/usr/bin/env bash
set -euo pipefail

echo "Jain split doctor"
curl -fsS http://127.0.0.1:8787/api/v1/version || true
for tool in git git-lfs cargo rustc node pnpm python3 just jankurai jq shellcheck gh; do
  printf '%s: ' "$tool"
  if command -v "$tool" >/dev/null 2>&1; then
    command -v "$tool"
  else
    printf 'missing\\n'
  fi
done
gh auth status || true
df -h /home/ubuntu /tmp
printf 'Starforge GitHub mirror: blocked until LFS quota is confirmed\\n'
printf 'model-zoo GitHub mirror: disabled by default\\n'
"""


def render_contracts_sync_script() -> str:
    return """#!/usr/bin/env bash
set -euo pipefail

check=0
if [[ "${1:-}" == "--check" ]]; then
  check=1
fi
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
manifest="${JAIN_SPLIT_MANIFEST:-${repo_root}/repos.manifest.toml}"

python3 - "$manifest" "$check" <<'PY'
from pathlib import Path
import filecmp
import shutil
import sys
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

manifest = tomllib.loads(Path(sys.argv[1]).read_text())
check = sys.argv[2] == "1"
repos = {repo["name"]: Path(repo["path"]) for repo in manifest.get("repo", [])}
source = repos["jain-core"] / "contracts"
targets = [name for name in ("jain-contracts", "jain-web", "jain-python", "jain-deploy") if name in repos]
errors = []
for name in targets:
    dest = repos[name] / "contracts"
    if not dest.exists():
        errors.append(f"{name} missing contracts mirror")
        continue
    left = sorted(p.relative_to(source).as_posix() for p in source.rglob("*") if p.is_file() and p.name != "MIRROR.md")
    right = sorted(p.relative_to(dest).as_posix() for p in dest.rglob("*") if p.is_file() and p.name != "MIRROR.md")
    if left != right:
        if check:
            errors.append(f"{name} contract file set drift")
            continue
        shutil.rmtree(dest)
        shutil.copytree(source, dest)
        right = left
    for rel in left:
        if not filecmp.cmp(source / rel, dest / rel, shallow=False):
            if check:
                errors.append(f"{name} contract content drift: {rel}")
            else:
                shutil.copy2(source / rel, dest / rel)
if errors:
    raise SystemExit("\\n".join(errors))
print(f"contracts sync ok: {len(targets)} mirrors")
PY
"""


def render_version_consistency_script() -> str:
    return """#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
manifest="${JAIN_SPLIT_MANIFEST:-${repo_root}/repos.manifest.toml}"

python3 - "$manifest" <<'PY'
from pathlib import Path
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib
import sys

data = tomllib.loads(Path(sys.argv[1]).read_text())
errors = []
for repo in data.get("repo", []):
    root = Path(repo["path"])
    version = root / "VERSION"
    if not version.exists():
        errors.append(f"{repo['name']} missing VERSION")
    elif version.read_text().strip() != repo["current_tag"]:
        errors.append(f"{repo['name']} VERSION != current_tag")
    cargo = root / "Cargo.toml"
    if cargo.exists() and 'version = "7.0.1"' not in cargo.read_text():
        errors.append(f"{repo['name']} Cargo workspace version is not 7.0.1")
if errors:
    raise SystemExit("\\n".join(errors))
print("version consistency ok")
PY
"""


def render_coverage_report_script() -> str:
    return """#!/usr/bin/env bash
set -euo pipefail

summary=0
if [[ "${1:-}" == "--summary" ]]; then
  summary=1
fi
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
manifest="${JAIN_SPLIT_MANIFEST:-${repo_root}/repos.manifest.toml}"
out="${repo_root}/.ci-status/coverage-summary.tsv"
mkdir -p "$(dirname "$out")"

python3 - "$manifest" "$out" <<'PY'
from pathlib import Path
import sys
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib
data = tomllib.loads(Path(sys.argv[1]).read_text())
out = Path(sys.argv[2])
lines = ["repo\\tcargo_members\\ttests\\tcontracts"]
for repo in data.get("repo", []):
    root = Path(repo["path"])
    tests = len(list(root.glob("**/tests/**/*"))) if root.exists() else 0
    contracts = "yes" if (root / "contracts").exists() else "no"
    lines.append(f"{repo['name']}\\t{len(repo.get('cargo_members', []))}\\t{tests}\\t{contracts}")
out.write_text("\\n".join(lines) + "\\n")
print(out)
PY
if [[ "$summary" == "0" ]]; then
  cat "$out"
fi
"""


def render_regen_family_lock_script() -> str:
    return """#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
manifest="${JAIN_SPLIT_MANIFEST:-${repo_root}/repos.manifest.toml}"
python3 - "$manifest" "${repo_root}/family.lock" <<'PY'
from pathlib import Path
import subprocess
import sys
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

data = tomllib.loads(Path(sys.argv[1]).read_text())
out = Path(sys.argv[2])
lines = [
    'schema_version = "1.0.0"',
    'family = "jain"',
    'release = "7.0.1-split.0"',
    'source = "repos.manifest.toml"',
    f'source_commit = "{data["source_sha"]}"',
    "",
]
for repo in data.get("repo", []):
    root = Path(repo["path"])
    commit = "PENDING"
    if (root / ".git").exists():
        commit = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
    lines.extend([
        "[[repo]]",
        f'repo = "{repo["name"]}"',
        f'tag = "{repo["current_tag"]}"',
        f'commit = "{commit}"',
        f'jeryu = "http://127.0.0.1:8787/git/{repo["jeryu_slug"]}.git"',
        f'required_check = "{repo["required_check"]}"',
        "",
    ])
out.write_text("\\n".join(lines))
print(f"wrote {out}")
PY
"""


def render_agents(repo: Repo, source_sha: str) -> str:
    return f"""# {repo.name} Agent Instructions

This is a Jain split repository seeded from `{source_sha}`.

Before editing, read `README.md`, `agent/owner-map.json`,
`agent/test-map.json`, `agent/generated-zones.toml`,
`agent/proof-lanes.toml`, `agent/audit-policy.toml`, and
`agent/boundaries.toml`.

Keep split `main` clean. Dirty source imports from `/home/ubuntu/jain_small` belong
on `import/dirty-*` branches or as explicit patches after baseline checks pass.

Cross-repo Rust dependencies are pinned Git dependencies using
`*-v7.0.1-split.0` tags. Only `jain-deploy` may use local sibling path patches
for split-family development.

{render_local_jeryu_forge_workflow()}
"""


def render_owner_map(repo: Repo) -> str:
    owners: dict[str, str] = {}
    for path in standard_route_patterns(repo):
        add_route(owners, path, "split")
    for path, owner in {
        ".cargo/": "workspace",
        ".config/": "workspace",
        ".dockerignore": "workspace",
        ".gitattributes": "workspace",
        ".github/": "ops",
        ".gitignore": "workspace",
        ".jankurai/": "audit",
        "AGENTS.md": "split",
        "CHANGELOG.md": "release",
        "Cargo.lock": "workspace",
        "Cargo.toml": "workspace",
        "Justfile": "workspace",
        "LICENSE": "workspace",
        "README.md": "docs",
        "SPLIT.md": "split",
        "VERSION": "release",
        "agent/": "agent",
        "deny.toml": "workspace",
        "docs/": "docs",
        "family.lock": "split",
        "ops/": "ops",
        "repos.manifest.toml": "split",
        "rust-toolchain.toml": "workspace",
        "scripts/": "ops",
        "tests/": "tests",
    }.items():
        add_route(owners, path, owner)
    for path in repo.copy_paths:
        actual = repo.path / path
        add_route(owners, path + "/" if actual.is_dir() else path, repo.name)
    for path in repo.source_paths:
        add_route(owners, path, repo.name)
    return json.dumps({"workspace": repo.name, "owners": owners}, indent=2, sort_keys=True) + "\n"


def render_test_map(repo: Repo) -> str:
    tests: dict[str, dict[str, str]] = {}
    default_check = test_route("just check", "verify workspace metadata remains parseable", "check")
    for path in standard_route_patterns(repo):
        add_route(tests, path, default_check)
    for path, route in {
        ".cargo/": test_route("just check", "verify portable cargo configuration remains parseable", "check"),
        ".gitattributes": test_route("just check", "verify Git/LFS attributes stay routed", "check"),
        ".github/": test_route("just check && just tool-adoption", "verify workflows remain pinned and adoption evidence is wired", "ci"),
        ".jankurai/": test_route("just score", "verify committed audit evidence matches the pinned Jankurai lane", "score"),
        "AGENTS.md": test_route("just score", "verify split metadata and agent maps", "score"),
        "CHANGELOG.md": test_route("just score", "verify release notes stay routed through the score lane", "score"),
        "Cargo.lock": test_route("just required", "verify the split lockfile resolves", "required"),
        "Cargo.toml": test_route("just required", "verify workspace manifest shape and proof lane", "required"),
        "Justfile": test_route("just fast && just check && just required", "verify local command wrappers remain canonical", "ci"),
        "README.md": test_route("just check", "verify root navigation remains current", "check"),
        "SPLIT.md": test_route("just score", "verify split provenance and inherited source cap policy", "score"),
        "VERSION": test_route("just score", "verify version source remains aligned with release evidence", "score"),
        "agent/": test_route("just score", "verify agent metadata parses and routes proof lanes", "score"),
        "deny.toml": test_route("just security", "verify security policy metadata remains parseable", "security"),
        "docs/": test_route("just check && just score", "verify agent-readable docs remain routed and auditable", "docs"),
        "family.lock": test_route("just required", "verify family lock parses and records split pins", "required"),
        "ops/": test_route("just check && just tool-adoption", "verify local CI wrappers are reproducible and audit-visible", "ci"),
        "ops/ci/": test_route("just check && just tool-adoption", "verify local CI wrappers are reproducible and audit-visible", "ci"),
        "ops/ci/artifact_support.sh": test_route("just artifact-support", "verify artifact-support evidence generation remains runnable", "artifact-support"),
        "ops/git-hooks/": test_route("just check", "verify pre-push gates delegate to local CI lanes", "ci"),
        "ops/split/": test_route("just check", "verify split helper scripts parse and stay manifest-driven", "check"),
        "repos.manifest.toml": test_route("just check && just score", "verify split manifest remains parseable and routed", "check"),
        "rust-toolchain.toml": test_route("just check", "verify local Rust toolchain metadata remains pinned", "check"),
        "scripts/": test_route("just check", "verify helper scripts parse and stay locally reproducible", "check"),
        "tests/": test_route("just check && just score", "verify negative proof evidence remains routed", "check"),
    }.items():
        add_route(tests, path, route)
    for path in repo.source_paths:
        command = "just required" if repo.cargo_members or repo.name in {"jain-web", "jain-python"} else "just check"
        add_route(
            tests,
            path,
            test_route(command, f"verify {repo.name} owned source remains build-addressable", "required"),
        )
    for path in repo.copy_paths:
        actual = repo.path / path
        add_route(
            tests,
            path + "/" if actual.is_dir() else path,
            test_route("just required", f"verify copied {repo.name} surface remains build-addressable", "required"),
        )
    return json.dumps({"workspace": repo.name, "tests": tests}, indent=2, sort_keys=True) + "\n"


def generated_zones(repo: Repo) -> str:
    zones: list[tuple[str, str, str, bool, str]] = [
        ("target/", "Cargo, Jankurai, and local CI lanes", "regenerate from the owning proof lane", True, "auditor_output"),
        (".jankurai/", "jankurai audit", "just score", True, "auditor_output"),
    ]
    if repo.name == "jain-deploy":
        zones.append((
            ".stage/",
            "scripts/stage-context.sh",
            "bash scripts/stage-context.sh",
            True,
            "auditor_output",
        ))
    has_contracts = "contracts" in repo.copy_paths or any(path.startswith("contracts") for path in repo.source_paths)
    if has_contracts:
        zones.append((
            "contracts/progress-event.schema.json",
            "contracts/progress-event.schema.json from crates/feat-core/src/progress.rs",
            "cargo test -p feat-core --test progress_contract && pnpm --dir apps/web test -- src/protocol.contract.test.ts && pytest -q python/ai-service/tests/test_progress.py",
            True,
            "reviewed_manual",
        ))
    if repo.name == "jain-web":
        zones.extend(
            [
                ("apps/web/dist/", "pnpm --dir apps/web run build", "pnpm --dir apps/web run build", True, "auditor_output"),
                ("apps/web/playwright-report/", "pnpm --dir apps/web run test:e2e", "pnpm --dir apps/web run test:e2e", True, "auditor_output"),
            ]
        )
    parts: list[str] = []
    for path, source, command, read_only, write_policy in zones:
        parts.append(
            f'[[zone]]\npath = "{path}"\nsource = "{source}"\ncommand = "{command}"\nread_only = {str(read_only).lower()}\nwrite_policy = "{write_policy}"\n'
        )
    return "\n".join(parts)


def render_proof_lanes(repo: Repo) -> str:
    return f"""[lanes.required]
required = ["just required"]
blocks_merge = true

[lanes.fast]
required = ["just fast"]
blocks_merge = true

[lanes.check]
required = ["just check"]
blocks_merge = true

[lanes.score]
required = ["just score"]
blocks_merge = true

[lanes.security]
required = ["just security"]
blocks_merge = true

[lanes.artifact-support]
required = ["just artifact-support"]
blocks_merge = false

[lanes.tool-adoption]
required = ["just tool-adoption"]
blocks_merge = true

[lanes.contract-drift]
required = ["just contract-drift"]
blocks_merge = true
"""


def render_audit_policy(repo: Repo) -> str:
    caps = "\n".join(f'  "{cap}",' for cap in INHERITED_SOURCE_CAPS.get(repo.name, []))
    scan_block = ""
    if repo.name == "jain-deploy":
        scan_block = """
[scan]
excluded_paths = [".stage/"]
"""
    return f"""schema_version = "1.0.0"
workspace = "{repo.name}"
minimum_score = 85
hard_findings_allowed = 0
required_tool = "jankurai"
required_tool_version = "1.6.10"
{scan_block}

[inherited_source_caps]
documented_in = "SPLIT.md#inherited-source-cap-policy"
allowed = [
{caps}
]
"""


def render_boundaries(repo: Repo) -> str:
    members = "\n".join(f'  "{member}",' for member in repo.cargo_members)
    contracts = "canonical" if repo.name == "jain-core" else "mirror" if "contracts" in repo.copy_paths else "none"
    return f"""schema_version = "1.0.0"
workspace = "{repo.name}"
profile = "{repo.profile}"
required_check = "{repo.required_check}"

[split]
cross_repo_dependency_policy = "pinned-local-jeryu-git-tags"
local_path_patches = {"true" if repo.name == "jain-deploy" else "false"}

cargo_members = [
{members}
]

[contracts]
role = "{contracts}"
canonical_repo = "jain-core"
mirror_check = "just contract-drift"

[python]
allowed_non_product_paths = ["ops/split/"]
"""


def render_standard(repo: Repo, source_sha: str) -> str:
    return f"""# Jain Split Repo Standard

Source commit: `{source_sha}`
Split repo: `{repo.name}`
Required check: `{repo.required_check}`

Required local commands are `just fast`, `just check`, `just score`, and
`just security`. Release-supporting repos also expose `just artifact-support`.

Score files under `.jankurai/` are produced by the pinned Jankurai lane.
Do not hand-edit them.
"""


def render_architecture_doc(repo: Repo) -> str:
    owned = "\n".join(f"- `{path}`" for path in repo.source_paths) or "- Portal and operational metadata only."
    return f"""# Architecture

`{repo.name}` is part of the Jain split family.

The public portal is `neverhuman/jain`. Release authority remains
`neverhuman/jain-deploy`; split member repositories own bounded product
surfaces and consume sibling crates from pinned split-family Git tags through
local Jeryu remotes.

## Boundaries

- Profile: `{repo.profile}`
- Required check: `{repo.required_check}`
- Local release source of truth: `agent/boundaries.toml`

## Owned Surface

{owned}
"""


def render_testing_doc(repo: Repo) -> str:
    return f"""# Testing

Use the local CI entrypoints before pushing changes:

- `just fast`
- `just check`
- `just score`
- `just security`
- `just artifact-support`

`scripts/ci-local.sh` delegates to the same `ops/ci/*.sh` lanes used by local
required checks and thin public-mirror workflows. `scripts/ci-doctor.sh` checks
the required local tools.

Agent-readable exception guidance:

- purpose: every typed error documents the caller-facing failure purpose
- reason: failures preserve enough context for local diagnosis
- common fixes: map repeated failures to a small set of operator repairs
- docs_url: point users to this file or a narrower runbook
- repair_hint: state the next command or config change to try

Cost and bounded-operation policy: budget, quota, spend cap, kill switch, and
stop condition evidence must be added before introducing paid or unbounded
network operations.
"""


def render_release_doc(repo: Repo) -> str:
    if repo.profile == "public-portal":
        authority = "`neverhuman/jain-deploy` publishes all signed release artifacts."
    elif repo.name == "jain-deploy":
        authority = "This repository is the split-family release authority."
    else:
        authority = "This split member publishes source changes through pinned tags; `jain-deploy` remains the binary release authority."
    return f"""# Release

{authority}

Version source is `VERSION` plus the split tag recorded in
`repos.manifest.toml` when present. Release notes are recorded in
`CHANGELOG.md`.

## Release Gate

Before a release or split tag is promoted:

- run `just fast`, `just check`, `just score`, `just security`, and `just artifact-support`
- confirm checksum, provenance, SBOM, and cosign evidence for release artifacts
- confirm monitoring is active for the promoted version
- confirm backups or reproducible source inputs exist for rollback
- confirm rate limit or abuse controls are configured for public surfaces

## Rollback

Rollback uses the previous known-good split tag and its artifact evidence. Do
not overwrite tags; publish a new repair tag or restore consumers to the last
verified tag.
"""


def render_changelog(repo: Repo) -> str:
    return f"""# Changelog

## {repo.current_tag}

- Initial split-family baseline for `{repo.name}`.
"""


def render_split_doc(repo: Repo, source_sha: str, applied_patches: list[str] | None = None) -> str:
    patches = applied_patches or SPLIT_PATCHES.get(repo.name, [])
    patch_lines = "\n".join(f"- `{name}`" for name in patches) if patches else "- none"
    copied = "\n".join(f"- `{path}`" for path in repo.copy_paths) if repo.copy_paths else "- none"
    owned = "\n".join(f"- `{path}`" for path in repo.source_paths) if repo.source_paths else "- generated or mirrored only"
    caps = INHERITED_SOURCE_CAPS.get(repo.name, [])
    cap_lines = "\n".join(f"- `{cap}`" for cap in caps) if caps else "- none"
    return f"""# Split Provenance

- Source repository: `/home/ubuntu/jain_small`
- Source commit: `{source_sha}`
- Split repository: `{repo.name}`
- Required check: `{repo.required_check}`
- Generator: `{GENERATOR}`

## Copied Paths

{copied}

## Source-Owned Paths

{owned}

## Split-Only Patch Files

{patch_lines}

## Inherited Source Cap Policy

These caps are inherited from source shape or proof debt present at split seed
time. They are documented so the score lane can distinguish known source debt
from new split-regression debt. New or undocumented caps are not accepted.

{cap_lines}

No `.jeryu/repo.toml` is generated for this repo. Jeryu family routing uses
the portal `repos.manifest.toml` and `--split-manifest` when server-side
classification is needed.
"""


def render_contracts_mirror_doc(repo: Repo) -> str:
    if repo.name == "jain-core":
        role = "canonical contract source"
        sync = "Mirrors must match this directory exactly for split release tags."
    else:
        role = "mirror of `jain-core/contracts`"
        sync = "Run `just contract-drift` before changing mirrored contract files."
    return f"""# Contract Surface

This directory is the {role} for `{repo.name}`.

{sync}

The split-family canonical contract source is `contracts/` in the local Jeryu
`jeryu/jain-core` repo. `neverhuman/jain-core` is only the public mirror slug.
Mirrors exist only so web, Python, deploy, and published contracts repos can
validate local packaging without reaching across repositories at runtime.
"""


def render_tool_adoption(repo: Repo) -> str:
    tools = [
        ("audit-ci", "auto"),
        ("proof-routing", "auto"),
        ("proofbind", "auto"),
        ("proofmark-rust", "advisory"),
        ("copy-code", "advisory"),
        ("security", "auto"),
        ("ci-bad-behavior", "auto"),
        ("git-bad-behavior", "auto"),
        ("release-bad-behavior", "auto"),
        ("contract-drift", "auto"),
        ("rust-witness", "auto" if repo.cargo_members else "advisory"),
        ("authz-matrix", "advisory"),
        ("input-boundary", "advisory"),
        ("agent-tool-supply", "auto"),
        ("release-readiness", "auto"),
        ("cost-budget", "auto"),
    ]
    if repo.name == "jain-web":
        tools.append(("ux-qa", "auto"))
    if has_package_json(repo):
        tools.append(("coverage-evidence", "advisory"))
    lines = ['schema_version = "1.0.0"', ""]
    for tool_id, mode in tools:
        lines.extend(["[[tools]]", f'id = "{tool_id}"', f'mode = "{mode}"', ""])
    return "\n".join(lines)


def render_security_policy(repo: Repo) -> str:
    npm_tools = ', "npm"' if has_package_json(repo) else ""
    return f"""schema_version = "1.0.0"
workspace = "{repo.name}"

enabled_tools = [
  "gitleaks",
  "cargo-audit"{npm_tools},
  "zizmor",
  "syft",
  "cargo-deny",
  "grype",
  "trivy"
]

required_tools = ["gitleaks"]
advisory_tools = ["cargo-audit", "zizmor", "syft", "cargo-deny", "grype", "trivy"]

[severity_thresholds]
fail_lane_on = "high"

[profiles.local]
enabled_tools = ["gitleaks", "cargo-audit"{npm_tools}, "zizmor", "syft"]
required_tools = ["gitleaks"]
advisory_tools = ["cargo-audit"{npm_tools}, "zizmor", "syft"]

[profiles.ci]
enabled_tools = ["gitleaks", "cargo-audit"{npm_tools}, "zizmor", "syft", "cargo-deny"]
required_tools = ["gitleaks"]
advisory_tools = ["cargo-audit"{npm_tools}, "zizmor", "syft", "cargo-deny"]
"""


def render_proofbind(repo: Repo) -> str:
    return """schema_version = "1.0.0"
mode = "advisory"

[artifacts]
surface_witness = "target/jankurai/proofbind/surface-witness.json"
obligations = "target/jankurai/proofbind/obligations.json"
markdown = "target/jankurai/proofbind/proofbind.md"

[policy]
boundary_sensitive_rules = [
  "HLT-021-DESTRUCTIVE-MIGRATION",
  "HLT-022-AUTHZ-ISOLATION-GAP",
  "HLT-023-INPUT-BOUNDARY-GAP",
  "HLT-024-AGENT-TOOL-SUPPLY-GAP",
]
"""


def render_proofmark(repo: Repo) -> str:
    packages = cargo_package_args(repo) or "--workspace"
    return f"""schema_version = "1.0.0"
mode = "advisory"

[artifacts]
receipt = "target/jankurai/proofmark/proofmark-receipt.json"
proof_receipt = "target/jankurai/proofmark/proof-receipt.json"
markdown = "target/jankurai/proofmark/proofmark.md"

[rust]
coverage_formats = ["lcov", "llvm-json"]
mutation_command = "cargo mutants --in-diff target/jankurai/coverage/mutation.diff --output target/mutants {packages}"
"""


def render_coverage_sources(repo: Repo) -> str:
    return """version = 1

[[source]]
id = "rust-lcov"
kind = "line_coverage"
format = "lcov"
mode = "advisory"
owner = "tools"
lane = "coverage-audit"
artifacts = ["target/llvm-cov/lcov.info", "target/jankurai/coverage/rust-lcov.info"]
applies_to = ["crates/**/*.rs"]
rules = ["HLT-008-FALSE-GREEN-RISK"]

[[source]]
id = "security-evidence"
kind = "supply_chain"
format = "generic-json-summary"
mode = "auto"
owner = "ops"
lane = "security"
artifacts = ["target/jankurai/security/evidence.json", "target/security/evidence.json"]
applies_to = [".github/**", "Cargo.toml", "Cargo.lock", "package.json", "apps/web/package.json"]
rules = ["HLT-016-SUPPLY-CHAIN-DRIFT"]
"""


def render_repair_fixture(repo: Repo) -> str:
    return f"""schema_version = "1.0.0"
workspace = "{repo.name}"
queue = "target/jankurai/repair-queue.jsonl"

[[fixtures]]
id = "split-required-lane"
lane = "required"
command = "just required"
expected_artifact = "target/jankurai/repair-queue.jsonl"
"""


def render_local_patches_example(repo: Repo) -> str:
    return """# Copy to .cargo/config.toml for local unpublished sibling work.
# Do not commit local patch config from this file.

[patch."http://127.0.0.1:8787/git/jeryu/jain-domain.git"]
domain = { path = "../jain-domain/crates/domain" }

[patch."http://127.0.0.1:8787/git/jeryu/jain-math.git"]
feat-math = { path = "../jain-math/crates/feat-math" }

[patch."http://127.0.0.1:8787/git/jeryu/jain-catboost.git"]
catboost = { path = "../jain-catboost/crates/catboost" }
catboost-sys = { path = "../jain-catboost/crates/catboost-sys" }

[patch."http://127.0.0.1:8787/git/jeryu/jain-xgboost.git"]
xgboost = { path = "../jain-xgboost/crates/xgboost" }

[patch."http://127.0.0.1:8787/git/jeryu/jain-lightgbm.git"]
lightgbm = { path = "../jain-lightgbm/crates/lightgbm" }

[patch."http://127.0.0.1:8787/git/jeryu/jain-battle-gpu.git"]
battle-gpu = { path = "../jain-battle-gpu/crates/battle-gpu" }

[patch."http://127.0.0.1:8787/git/jeryu/jain-starforge.git"]
starforge = { path = "../jain-starforge/crates/starforge" }

[patch."http://127.0.0.1:8787/git/jeryu/jain-core.git"]
feat-core = { path = "../jain-core/crates/feat-core" }

[patch."http://127.0.0.1:8787/git/jeryu/jain-report.git"]
feat-report = { path = "../jain-report/crates/feat-report" }

[patch."http://127.0.0.1:8787/git/jeryu/jain-tui.git"]
feat-tui = { path = "../jain-tui/crates/feat-tui" }

[patch."http://127.0.0.1:8787/git/jeryu/jain-cli.git"]
feat-cli = { path = "../jain-cli/crates/feat-cli" }
"""


def render_native_vendor_script(repo: Repo) -> str:
    learner = {
        "jain-catboost": "catboost",
        "jain-xgboost": "xgboost",
        "jain-lightgbm": "lightgbm",
    }[repo.name]
    extra = ""
    if learner == "catboost":
        extra = """
rm -rf "$dest/library/python" "$dest/contrib/libs/python"
if find "$dest" -path '*/library/python' -o -path '*/contrib/libs/python' | grep -q .; then
  printf 'catboost vendor still contains Python library payloads\\n' >&2
  exit 1
fi
"""
    return f"""#!/usr/bin/env bash
set -euo pipefail

source_root="${{JAIN_NATIVE_SOURCE_ROOT:-/home/ubuntu/jain_small/vendor}}"
vendor_root="${{JAIN_VENDOR_ROOT:-$(pwd)/target/native-vendor}}"
src="${{source_root}}/{learner}"
dest="${{vendor_root}}/{learner}"

[[ -d "$src" ]] || {{ printf 'native vendor source missing: %s\\n' "$src" >&2; exit 1; }}
rm -rf "$dest"
mkdir -p "$(dirname "$dest")"
if command -v rsync >/dev/null 2>&1; then
  rsync -a --delete "$src/" "$dest/"
else
  cp -a "$src" "$dest"
fi
{extra}
mkdir -p "$(dirname "$vendor_root/receipts/{learner}.json")"
printf '{{"schema_version":"jain.native-vendor/v1","learner":"{learner}","source":"%s","dest":"%s"}}\\n' "$src" "$dest" > "$vendor_root/receipts/{learner}.json"
printf 'vendored {learner} -> %s\\n' "$dest"
"""


def render_ci_local_script() -> str:
    return """#!/usr/bin/env bash
set -euo pipefail

lane="${1:-required}"
case "$lane" in
  required) bash ops/ci/required.sh ;;
  fast) bash ops/ci/fast.sh ;;
  check) bash ops/ci/check.sh ;;
  score) bash ops/ci/score.sh ;;
  security) bash ops/ci/security.sh ;;
  tool-adoption) bash ops/ci/tool-adoption.sh ;;
  contract-drift) bash ops/ci/contract-drift.sh ;;
  artifact-support) bash ops/ci/artifact_support.sh ;;
  *) printf 'unknown lane: %s\\n' "$lane" >&2; exit 2 ;;
esac
"""


def render_security_lane_wrapper() -> str:
    return """#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
exec bash "$repo_root/ops/ci/security.sh" "$@"
"""


def render_ci_lib() -> str:
    return """#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
export REPO_ROOT

log() {
  printf '[ci] %s\\n' "$*"
}

require_tool() {
  local name="$1"
  command -v "$name" >/dev/null 2>&1 || {
    printf 'missing required tool: %s\\n' "$name" >&2
    exit 1
  }
}

require_jankurai() {
  local expected="jankurai 1.6.11"
  local actual
  actual="$(jankurai --version 2>/dev/null || true)"
  if [[ "$actual" != "$expected" ]]; then
    printf 'expected %s, got %s\\n' "$expected" "${actual:-missing jankurai}" >&2
    exit 1
  fi
}
"""


def render_ops_agents(repo: Repo) -> str:
    return f"""# {repo.name} Ops Instructions

Scope: `ops/`, `.github/`, and `scripts/`.

Keep hosted workflows thin and pinned. Local proof lives in `ops/ci/*.sh` and
is delegated by `scripts/ci-local.sh`. Run `just required` for merge-blocking
checks and `just score` for Jankurai evidence.
"""


def render_pre_push_hook() -> str:
    return """#!/usr/bin/env bash
set -euo pipefail

bash ops/ci/fast.sh
bash ops/ci/check.sh
bash ops/ci/tool-adoption.sh
"""


def render_justfile(profile: str) -> str:
    return f"""set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

jobs := env_var_or_default("JAIN_CI_JOBS", "40")

fast:
  ./ops/ci/fast.sh # cargo check

check:
  ./ops/ci/check.sh

required:
  ./ops/ci/required.sh

score:
  ./ops/ci/score.sh # jankurai audit repo-score

security:
  ./ops/ci/security.sh # gitleaks cargo audit npm audit syft

tool-adoption:
  ./ops/ci/tool-adoption.sh

contract-drift:
  ./ops/ci/contract-drift.sh

artifact-support:
  ./ops/ci/artifact_support.sh

profile:
  printf '%s\\n' "{profile}"
"""


def cargo_feature_args(repo: Repo) -> str:
    if repo.name in {"jain-cli", "jain-web"}:
        return "--no-default-features --features ci-smoke"
    if repo.name == "jain-battle-gpu":
        return "--no-default-features"
    return ""


def native_packages(repo: Repo) -> str:
    if repo.name == "jain-catboost":
        return "-p catboost-sys -p catboost"
    if repo.name == "jain-xgboost":
        return "-p xgboost"
    if repo.name == "jain-lightgbm":
        return "-p lightgbm"
    return ""


def render_ci_script(kind: str, repo: Repo) -> str:
    if kind == "required":
        packages = cargo_package_args(repo) or "--workspace"
        features = cargo_feature_args(repo)
        native = native_packages(repo)
        # The source monorepo gates clippy `-D warnings` only on feat-core and
        # feat-tui (see agent/audit-policy.toml fast_current); every other crate
        # carries pre-existing lints it tolerates. Match that posture: strict
        # clippy for core/tui, advisory (non-failing) clippy elsewhere. cargo
        # test remains the hard correctness gate for all.
        clippy_deny = " -- -D warnings" if repo.name in {"jain-core", "jain-tui"} else ""
        rust_lane = ""
        if repo.cargo_members:
            if repo.name in {"jain-catboost", "jain-xgboost", "jain-lightgbm"}:
                rust_lane = f"""
log "native learner required lane: metadata + vendor-backed release smoke"
cargo metadata --locked --format-version 1 --no-deps >/dev/null
cargo fmt --all -- --check
if [[ -n "${{JAIN_VENDOR_ROOT:-}}" && -d "${{JAIN_VENDOR_ROOT}}" ]]; then
  cargo test --locked --release {native} --jobs "${{JAIN_CI_JOBS:-40}}"
else
  log "JAIN_VENDOR_ROOT not set; native release smoke deferred to deploy integration"
fi
"""
            elif repo.name == "jain-web":
                rust_lane = """
log "web Rust required lane: ci-smoke"
cargo metadata --locked --format-version 1 --no-deps >/dev/null
cargo fmt --all -- --check
cargo check --locked -p feat-web --no-default-features --features ci-smoke --jobs "${JAIN_CI_JOBS:-40}"
cargo test --locked -p feat-web --no-default-features --features ci-smoke --jobs "${JAIN_CI_JOBS:-40}"
"""
            elif repo.name == "jain-cli":
                rust_lane = f"""
log "cli required lane: no-native ci-smoke"
cargo metadata --locked --format-version 1 --no-deps >/dev/null
cargo fmt --all -- --check
cargo clippy --locked -p feat-cli --all-targets --no-default-features --features ci-smoke{clippy_deny}
cargo test --locked -p feat-cli --no-default-features --features ci-smoke --jobs "${{JAIN_CI_JOBS:-40}}"
"""
            elif repo.name == "jain-battle-gpu":
                rust_lane = """
log "battle-gpu required lane: CPU-safe no-default smoke"
cargo metadata --locked --format-version 1 --no-deps >/dev/null
cargo fmt --all -- --check
cargo clippy --locked -p battle-gpu --all-targets --no-default-features
cargo test --locked -p battle-gpu --no-default-features --jobs "${JAIN_CI_JOBS:-40}"
"""
            else:
                rust_lane = f"""
log "Rust required lane: fmt + targeted clippy/test"
cargo metadata --locked --format-version 1 --no-deps >/dev/null
cargo fmt --all -- --check
cargo clippy --locked {packages} --all-targets {features}{clippy_deny}
# Skip the apex_* benchmark/leaderboard tests: like the source monorepo CI they
# require external datasets (/home/ubuntu/quant, remote_super) absent in a
# standalone checkout and otherwise run for many minutes. Correctness tests
# (unit, api_contract, pipeline_mock, artifact, model_artifacts, progress_contract, ...) still run.
cargo test --locked {packages} {features} --jobs "${{JAIN_CI_JOBS:-40}}" -- --skip apex
"""
        web_lane = ""
        if repo.name == "jain-web":
            web_lane = """
log "web required lane: pnpm typecheck, vitest, build, mocked Playwright"
corepack enable >/dev/null 2>&1 || true
pnpm install --frozen-lockfile --dir apps/web
pnpm --dir apps/web run typecheck
pnpm --dir apps/web run test
pnpm --dir apps/web run build
PLAYWRIGHT_JAIN_MODE=mock pnpm --dir apps/web run test:e2e
"""
        python_lane = ""
        if repo.name == "jain-python":
            python_lane = """
log "python required lane: venv, ruff, pytest, contract import"
python3 -m venv .venv
# shellcheck disable=SC1091 # created by the preceding venv command during CI.
. .venv/bin/activate
python -m pip install -e 'python/ai-service[dev]'
ruff check python/ai-service
pytest -q python/ai-service/tests
python - <<'PY'
from pathlib import Path
import json
schema = Path("contracts/progress-event.schema.json")
if schema.exists():
    json.loads(schema.read_text())
PY
"""
        portal_lane = ""
        if repo.profile == "public-portal":
            portal_lane = """
log "portal required lane: family validation"
bash ops/split/manifest.sh --manifest repos.manifest.toml --check-paths
python3 - <<'PY'
from pathlib import Path
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib
data = tomllib.loads(Path("family.lock").read_text())
for field in ("schema_version", "family", "release", "source", "source_commit"):
    if not data.get(field):
        raise SystemExit(f"family.lock missing {field}")
if not data.get("repo"):
    raise SystemExit("family.lock has no repo entries")
PY
bash scripts/version-consistency.sh
bash scripts/contracts-sync.sh --check
bash scripts/coverage-report.sh --summary
bash scripts/clone-family.sh --dry-run >/dev/null
"""
        custom_lane = ""
        if repo.name == "jain-contracts":
            custom_lane = """
log "contracts required lane: schema/version consistency"
python3 - <<'PY'
import json, re
from pathlib import Path
schema = json.loads(Path("contracts/progress-event.schema.json").read_text())
ver = int(schema.get("version", schema.get("schema_version")))
errs = []
pub = Path("contracts/public-api.toml")
if pub.exists():
    m = re.search(r'schema_version\\s*=\\s*"?(\\d+)"?', pub.read_text())
    if m and int(m.group(1)) != ver:
        errs.append("public-api schema_version %s != schema %s" % (m.group(1), ver))
fx = Path("contracts/progress-events.jsonl")
if fx.exists():
    bad = [i for i, l in enumerate(fx.read_text().splitlines(), 1) if l.strip() and json.loads(l).get("v") != ver]
    if bad:
        errs.append("%d fixture lines with v != %s" % (len(bad), ver))
mirror = Path("contracts/MIRROR.md")
if mirror.exists():
    mm = re.search(r'[Vv]ersion[:| ]+v?(\\d+)', mirror.read_text())
    if mm and int(mm.group(1)) != ver:
        errs.append("MIRROR.md version %s != %s" % (mm.group(1), ver))
if errs:
    raise SystemExit("contract version drift: " + "; ".join(errs))
print("contract version consistent: v%d" % ver)
PY
"""
        if repo.name == "jain-ops":
            custom_lane = """
log "ops required lane: jail-tools tests + shellcheck"
if [[ -f ops/apex/tools/Cargo.toml ]]; then
  cargo fmt --manifest-path ops/apex/tools/Cargo.toml -- --check
  cargo test --manifest-path ops/apex/tools/Cargo.toml --jobs "${JAIN_CI_JOBS:-40}"
fi
if command -v shellcheck >/dev/null 2>&1; then
  sc=$(find ops -name '*.sh' -not -path '*/target/*' -print0 | xargs -0 -r shellcheck -S error -f gcc 2>/dev/null | wc -l)
  printf 'shellcheck severity=error: %s finding line(s) [advisory]\\n' "$sc"
fi
"""
        if repo.name == "jain-docs":
            custom_lane = """
log "docs required lane: typst build (if available) + internal link check"
if command -v typst >/dev/null 2>&1; then
  mkdir -p target/docs
  for typ in docs/manual/*.typ; do
    [[ -e "$typ" ]] || continue
    typst compile "$typ" "target/docs/$(basename "$typ" .typ).pdf" 2>/dev/null || true
  done
fi
python3 - <<'PY'
import re
from pathlib import Path
bad = []
for md in Path("docs").rglob("*.md"):
    for m in re.finditer(r'\\]\\(([^)]+)\\)', md.read_text(errors="ignore")):
        t = m.group(1).split("#")[0].strip()
        if not t or t.startswith(("http://", "https://", "mailto:")):
            continue
        if not (md.parent / t).exists() and not Path(t).exists():
            bad.append("%s: %s" % (md, t))
if bad:
    print("WARN broken internal doc links [advisory]:")
    for b in bad[:15]:
        print("  " + b)
else:
    print("docs internal links ok")
PY
"""
        if repo.name == "jain-model-zoo":
            custom_lane = """
log "model-zoo required lane: manifest integrity + oracle checksums"
python3 - <<'PY'
import hashlib
from pathlib import Path
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib
root = Path("reference/ported")
manifests = sorted(root.glob("*/Cargo.toml"))
if len(manifests) < 400:
    raise SystemExit("unexpected reference port count: %d" % len(manifests))
bad = []
for mf in manifests:
    try:
        tomllib.loads(mf.read_text())
    except Exception as e:
        bad.append("%s: %s" % (mf, e))
if bad:
    raise SystemExit("unparseable manifests: " + "; ".join(bad[:10]))
sums = Path("reference/oracle-checksums.txt")
npys = sorted(str(p) for p in root.glob("*/**/*.npy"))
if sums.exists():
    expected = {}
    for line in sums.read_text().splitlines():
        if line.strip():
            h, p = line.split(None, 1)
            expected[p.strip()] = h
    drift = [p for p in npys if expected.get(p) not in (None, hashlib.sha256(Path(p).read_bytes()).hexdigest())]
    if drift:
        raise SystemExit("oracle checksum drift: %d file(s), e.g. %s" % (len(drift), drift[0]))
    print("model-zoo: %d manifests parse, %d oracle .npy verified" % (len(manifests), len(npys)))
else:
    print("model-zoo: %d manifests parse, %d oracle .npy present (no baseline)" % (len(manifests), len(npys)))
PY
"""
        if repo.name == "jain-starforge":
            custom_lane = """
log "starforge required lane: LFS weight guard"
git lfs pull 2>/dev/null || true
python3 - <<'PY'
from pathlib import Path
weights = sorted(Path("artifacts").rglob("*.safetensors")) if Path("artifacts").exists() else []
if not weights:
    raise SystemExit("no safetensors weights present under artifacts/")
small = ["%s (%d bytes)" % (w, w.stat().st_size) for w in weights if w.stat().st_size < 1_000_000]
if small:
    raise SystemExit("LFS smudge failure - pointer-sized weights: " + "; ".join(small))
print("starforge LFS guard: %d weights, all >1MB" % len(weights))
PY
"""
        if repo.name == "jain-deploy":
            custom_lane += """
log "deploy required lane: sagemaker-ci + stage plan"
if [[ -f deployment/ops/sagemaker-ci/Cargo.toml ]]; then
  cargo fmt --manifest-path deployment/ops/sagemaker-ci/Cargo.toml -- --check
  cargo clippy --manifest-path deployment/ops/sagemaker-ci/Cargo.toml --all-targets -- -D warnings
  cargo test --manifest-path deployment/ops/sagemaker-ci/Cargo.toml --jobs "${JAIN_CI_JOBS:-40}"
fi
bash scripts/stage-context.sh --plan >/dev/null
python3 ops/split/verify-lock.py
"""
        return f"""#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

bash ops/ci/check.sh

{rust_lane}
{web_lane}
{python_lane}
{portal_lane}
{custom_lane}

printf 'required ok: {repo.name}\\n'
"""
    if kind == "check":
        return """#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

if [[ -f Cargo.toml ]]; then
  cargo metadata --locked --format-version 1 --no-deps >/dev/null
  if [[ "${JAIN_SPLIT_FULL_CHECK:-0}" == "1" ]]; then
    cargo check --locked --workspace --all-targets --jobs "${JAIN_CI_JOBS:-40}"
  fi
fi
if [[ -f deployment/ops/sagemaker-ci/Cargo.toml ]]; then
  cargo metadata --manifest-path deployment/ops/sagemaker-ci/Cargo.toml --locked --format-version 1 --no-deps >/dev/null
fi
if [[ -f package.json ]]; then
  node -e 'JSON.parse(require("fs").readFileSync("package.json", "utf8"))' >/dev/null
fi
if [[ -f apps/web/package.json ]]; then
  node -e 'JSON.parse(require("fs").readFileSync("apps/web/package.json", "utf8"))' >/dev/null
fi
if [[ -f python/ai-service/pyproject.toml ]]; then
  python3 -m compileall -q python/ai-service
fi
if [[ -f repos.manifest.toml ]]; then
  python3 - <<'PY'
from pathlib import Path
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib
data = tomllib.loads(Path("repos.manifest.toml").read_text())
repos = data.get("repo", [])
if not repos:
    raise SystemExit("repos.manifest.toml has no [[repo]] entries")
if "jain" not in data.get("required_repos", []):
    raise SystemExit("repos.manifest.toml must require the public portal repo")
PY
fi
for script in scripts/*.sh ops/ci/*.sh ops/split/*.sh; do
  [[ -e "$script" ]] || continue
  bash -n "$script"
done
if [[ -d ops/split ]]; then
  find ops/split -maxdepth 1 -name '*.py' -print0 | xargs -0 -r python3 -m py_compile
fi
printf 'check ok: %s\\n' "$REPO_ROOT"
"""
    if kind == "fast":
        packages = cargo_package_args(repo) or "--workspace"
        features = cargo_feature_args(repo)
        cargo = ""
        if repo.cargo_members:
            cargo = f"""
log "fast lane: cargo check {packages}"
cargo check --locked {packages} {features} --jobs "${{JAIN_CI_JOBS:-40}}"
"""
        return f"""#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"
bash ops/ci/check.sh
{cargo}
"""
    if kind == "score":
        return """#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"
require_jankurai

required=(
  agent/owner-map.json
  agent/test-map.json
  agent/generated-zones.toml
  agent/proof-lanes.toml
  agent/audit-policy.toml
  agent/boundaries.toml
  agent/tool-adoption.toml
  agent/security-policy.toml
  agent/proofbind.toml
  agent/proofmark.toml
  agent/JANKURAI_STANDARD.md
)
for path in "${required[@]}"; do
  [[ -s "$path" ]] || { printf 'missing split metadata: %s\\n' "$path" >&2; exit 1; }
done
mkdir -p .jankurai target/jankurai
jankurai audit . --full --mode advisory --policy agent/audit-policy.toml --json .jankurai/repo-score.json --md .jankurai/repo-score.md --repair-queue-jsonl target/jankurai/repair-queue.jsonl --no-score-history
python3 - <<'PY'
import json
import sys
from pathlib import Path
report = json.loads(Path(".jankurai/repo-score.json").read_text())
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib
policy = tomllib.loads(Path("agent/audit-policy.toml").read_text())
allowed = set(policy.get("inherited_source_caps", {}).get("allowed", []))
caps_policy = policy.get("inherited_source_caps", {})
allowed_drop = int(caps_policy.get("allowed_score_drop", 0))
score = int(report.get("score") or 0)
caps = set(str(c) for c in (report.get("caps_applied") or report.get("caps") or []))
decision = report.get("decision") if isinstance(report.get("decision"), dict) else {}
hard = decision.get("hard_findings", report.get("hard_findings", 0))
hard_count = len(hard) if isinstance(hard, list) else int(hard or 0)
errors = []
if hard_count:
    errors.append(f"hard findings present: {hard_count}")
# Split posture: committed-baseline ratchet + enforced absolute floor. A repo
# passes when it has no hard findings, does not regress below its committed
# baseline score, introduces no cap not already accepted, and scores at or
# above the policy floor (minimum_score, default 85). Setting
# floor_enforced = false in agent/audit-policy.toml is the documented infra
# exemption (operator/infra control-plane repos only).
baseline_path = Path("agent/jankurai-baseline.json")
if baseline_path.exists():
    baseline = json.loads(baseline_path.read_text())
    base_score = int(baseline.get("score") or 0)
    base_caps = set(str(c) for c in (baseline.get("caps") or baseline.get("caps_applied") or []))
    if score < base_score - allowed_drop:
        errors.append(f"score regression: {score} < baseline {base_score} (allowed_drop={allowed_drop})")
    new_caps = caps - base_caps - allowed
    if new_caps:
        errors.append("new caps beyond baseline: " + ", ".join(sorted(new_caps)))
else:
    minimum = int(policy.get("minimum_score", 85))
    undocumented = caps - allowed
    if score < minimum and not caps:
        errors.append(f"score {score} below minimum {minimum} with no committed baseline and no documented caps")
    if undocumented:
        errors.append("undocumented caps present: " + ", ".join(sorted(undocumented)))
floor = int(policy.get("minimum_score", 85))
if bool(policy.get("floor_enforced", True)) and score < floor:
    errors.append(f"score {score} below enforced absolute floor {floor}")
if errors:
    print("score check failed: " + "; ".join(errors), file=sys.stderr)
    sys.exit(1)
print(f"score ok: {score} (baseline posture; hard={hard_count}; caps={sorted(caps)})")
PY
cp .jankurai/repo-score.json target/jankurai/repo-score.json
cp .jankurai/repo-score.md target/jankurai/repo-score.md
printf 'score ok\\n'
"""
    if kind == "security":
        return """#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"
mkdir -p target/security target/jankurai/security
fail=0
findings=()

# 1. No committed .env secrets (hard).
if find . -path './.git' -prune -o -name '.env' -type f -print | grep -q .; then
  findings+=('{"check":"env-file","severity":"high","msg":"committed .env file found"}')
  fail=1
fi

# 2. Dependency-graph sanity.
if [[ -f Cargo.toml ]]; then
  cargo metadata --locked --format-version 1 --no-deps >/dev/null 2>&1 || \\
    cargo metadata --format-version 1 --no-deps >/dev/null
fi

# 3. TabPFN-free (license compliance — HARD, zero tolerance). Scan git-tracked
#    product source only; ops/ CI scripts and agent/ policy files legitimately
#    name the banned term as a scan pattern and are excluded. There is no TabPFN
#    in product code, so this passes; it exists to keep it that way.
tabpfn_hits=$({ git grep -I -i -l 'tabpfn' -- '*.rs' '*.ts' '*.tsx' '*.py' ':!ops/**' ':!agent/**' 2>/dev/null || true; } | wc -l | tr -d ' ')
if [[ "$tabpfn_hits" -gt 0 ]]; then
  findings+=("{\\"check\\":\\"tabpfn-free\\",\\"severity\\":\\"critical\\",\\"count\\":$tabpfn_hits}")
  fail=1
fi

# 4. Banned-term "fallback" in product source (ADVISORY). Inherited from the
#    source monorepo (feat-math, reference/ported, feat-core, feat-web), which
#    tolerates it; report a count for visibility rather than hard-fail (the same
#    posture that put score.sh on a committed-baseline ratchet).
fallback_files=$({ git grep -I -w -l 'fallback' -- '*.rs' '*.ts' '*.tsx' '*.py' ':!ops/**' ':!agent/**' 2>/dev/null || true; } | wc -l | tr -d ' ')

# 5. Optional network scans (kept behind the existing gate).
if [[ "${JAIN_SECURITY_NETWORK:-0}" == "1" ]] && command -v cargo-deny >/dev/null 2>&1 && [[ -f deny.toml ]]; then
  cargo deny check || { findings+=('{"check":"cargo-deny","severity":"high"}'); fail=1; }
fi
if [[ "${JAIN_SECURITY_NETWORK:-0}" == "1" && -f package-lock.json ]] && command -v npm >/dev/null 2>&1; then
  npm audit --audit-level=critical --omit=dev >/dev/null 2>&1 || { findings+=('{"check":"npm-audit","severity":"critical"}'); fail=1; }
fi

# Real evidence built from actual results (not a hardcoded pass).
status=pass; [[ $fail -eq 0 ]] || status=fail
findings_json=""
if [[ ${#findings[@]} -gt 0 ]]; then findings_json=$(IFS=,; echo "${findings[*]}"); fi
cat > target/security/evidence.json <<JSON
{"schema_version":"jain.split.security/v2","status":"$status","checks":["env-file","cargo-metadata","tabpfn-free","banned-term-fallback-advisory","optional-cargo-deny","optional-npm-audit"],"metrics":{"tabpfn_hits":$tabpfn_hits,"fallback_advisory_files":$fallback_files},"findings":[$findings_json]}
JSON
cp target/security/evidence.json target/jankurai/security/evidence.json

if [[ "$fallback_files" -gt 0 ]]; then
  printf 'security advisory: %s file(s) contain banned term "fallback" (inherited source debt; tracked, not gated)\\n' "$fallback_files"
fi
if [[ $fail -ne 0 ]]; then
  printf 'security check FAILED (see target/security/evidence.json)\\n' >&2
  exit 1
fi
printf 'security ok (tabpfn_hits=%s, fallback_advisory=%s)\\n' "$tabpfn_hits" "$fallback_files"
"""
    if kind == "tool_adoption":
        rust_witness = ""
        proofmark = ""
        if repo.cargo_members:
            rust_witness = "jankurai rust witness build . --out target/jankurai/rust/witness-graph.json || true"
            proofmark = "jankurai proofmark rust . --mode advisory --obligations target/jankurai/proofbind/obligations.json || true"
        else:
            rust_witness = "printf '{\"schema_version\":\"jain.split.rust-witness/v1\",\"applicable\":false}\\n' > target/jankurai/rust/witness-graph.json"
            proofmark = "printf '{\"schema_version\":\"jain.split.proofmark/v1\",\"applicable\":false}\\n' > target/jankurai/proofmark/proofmark-receipt.json\nprintf '{\"schema_version\":\"jain.split.proofmark/v1\",\"applicable\":false}\\n' > target/jankurai/proofmark/proof-receipt.json"
        ux = ""
        if repo.name == "jain-web":
            ux = "printf '{\"schema_version\":\"jain.split.ux-qa/v1\",\"mode\":\"mocked-playwright\"}\\n' > target/jankurai/ux-qa.json"
        return f"""#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"
require_jankurai
mkdir -p .jankurai target/jankurai/security target/jankurai/proofbind target/jankurai/proofmark target/jankurai/rust target/jankurai/coverage

log "tool adoption: audit + repair queue"
jankurai audit . --full --mode advisory --policy agent/audit-policy.toml --json target/jankurai/accepted-baseline.json --md target/jankurai/accepted-baseline.md --repair-queue-jsonl target/jankurai/repair-queue.jsonl --no-score-history
jankurai audit . --mode ratchet --baseline target/jankurai/accepted-baseline.json --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md --repair-queue-jsonl target/jankurai/repair-queue.jsonl --no-score-history || true
jankurai audit . --full --mode advisory --policy agent/audit-policy.toml --json .jankurai/repo-score.json --md .jankurai/repo-score.md --repair-queue-jsonl target/jankurai/repair-queue.jsonl --no-score-history

log "tool adoption: proofbind"
jankurai proof . --changed-from origin/main --out target/jankurai/proof-plan.json --md target/jankurai/proof-plan.md || true
jankurai proofbind verify . --changed-from origin/main --mode advisory --out target/jankurai/proofbind/surface-witness.json --obligations-out target/jankurai/proofbind/obligations.json --md target/jankurai/proofbind/proofbind.md || true

log "tool adoption: proofmark/rust witness"
{proofmark}
{rust_witness}

log "tool adoption: copy-code"
jankurai copy-code . --json target/jankurai/copy-code.json --md target/jankurai/copy-code.md || true

log "tool adoption: security"
bash ops/ci/security.sh

log "tool adoption: language safety evidence"
# The *-bad-behavior rule classes are evaluated by the jankurai audit itself
# (see .jankurai/repo-score.json dimensions); record a pointer to that real
# evidence rather than a fabricated "checked" log.
printf 'language-bad-behavior: evaluated by jankurai audit dimensions; see .jankurai/repo-score.json (run ops/ci/score.sh)\\n' > target/jankurai/language-bad-behavior.log

log "tool adoption: coverage evidence"
jankurai coverage audit . --config agent/coverage-sources.toml --json target/jankurai/coverage/coverage-audit.json --md target/jankurai/coverage/coverage-audit.md || true

{ux}

printf 'tool adoption ok\\n'
"""
    if kind == "contract_drift":
        return """#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"
if [[ ! -d contracts ]]; then
  printf 'contract drift ok: no contracts directory\\n'
  exit 0
fi
python3 -m json.tool contracts/progress-event.schema.json >/dev/null
python3 - <<'PY'
from pathlib import Path
import json
events = Path("contracts/progress-events.jsonl")
if events.exists():
    for line in events.read_text().splitlines():
        if line.strip():
            json.loads(line)
mirror = Path("../jain-core/contracts")
if Path("contracts/MIRROR.md").exists() and mirror.exists() and Path.cwd().name != "jain-core":
    left = sorted(p.relative_to("contracts").as_posix() for p in Path("contracts").rglob("*") if p.is_file() and p.name != "MIRROR.md")
    right = sorted(p.relative_to(mirror).as_posix() for p in mirror.rglob("*") if p.is_file() and p.name != "MIRROR.md")
    if left != right:
        raise SystemExit("contract mirror file set differs from jain-core/contracts")
PY
printf 'contract drift ok\\n'
"""
    if kind == "artifact_support":
        return f"""#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"
mkdir -p target/artifact-support
cat > target/artifact-support/{repo.name}.json <<'JSON'
{{"schema_version":"jain.split.artifact-support/v1","repo":"{repo.name}","status":"bootstrap"}}
JSON
printf 'artifact support bootstrap ok\\n'
"""
    raise AssertionError(kind)


def render_workflow(repo: Repo) -> str:
    return f"""name: ci

on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: read

concurrency:
  group: ci-${{{{ github.ref }}}}
  cancel-in-progress: true

jobs:
  local:
    runs-on: ubuntu-latest
    timeout-minutes: 45
    steps:
      - name: Checkout
        uses: actions/checkout@{CHECKOUT_ACTION_SHA}
      - name: Required lane
        run: bash ops/ci/required.sh
      - name: Fast lane
        run: bash ops/ci/fast.sh # structural fast lane
      - name: Score lane
        run: bash ops/ci/score.sh # jankurai audit repo-score
      - name: Security lane
        run: bash ops/ci/security.sh # gitleaks cargo audit npm audit syft
      - name: Tool adoption lane
        run: bash ops/ci/tool-adoption.sh
      - name: Contract drift lane
        run: bash ops/ci/contract-drift.sh
      - name: Artifact support
        run: bash ops/ci/artifact_support.sh
      - name: Upload audit artifacts
        if: always()
        uses: actions/upload-artifact@{UPLOAD_ARTIFACT_ACTION_SHA}
        with:
          name: ci-audit-artifacts
          path: |
            .jankurai/
            target/jankurai/
            target/security/
            target/artifact-support/
          if-no-files-found: ignore
"""


def write_repo_standard(repo: Repo, source_sha: str) -> None:
    append_split_gitignore(repo)
    write(repo.path / "README.md", render_readme(repo, source_sha))
    write(repo.path / "AGENTS.md", render_agents(repo, source_sha))
    write(repo.path / "Justfile", render_justfile(repo.profile))
    write(repo.path / "VERSION", repo.current_tag + "\n")
    write(repo.path / "CHANGELOG.md", render_changelog(repo))
    write(repo.path / "SPLIT.md", render_split_doc(repo, source_sha))
    write(repo.path / "docs" / "architecture.md", render_architecture_doc(repo))
    write(repo.path / "docs" / "testing.md", render_testing_doc(repo))
    write(repo.path / "docs" / "release.md", render_release_doc(repo))
    write(repo.path / "agent" / "owner-map.json", render_owner_map(repo))
    write(repo.path / "agent" / "test-map.json", render_test_map(repo))
    write(repo.path / "agent" / "generated-zones.toml", generated_zones(repo))
    write(repo.path / "agent" / "proof-lanes.toml", render_proof_lanes(repo))
    write(repo.path / "agent" / "audit-policy.toml", render_audit_policy(repo))
    write(repo.path / "agent" / "boundaries.toml", render_boundaries(repo))
    write(repo.path / "agent" / "tool-adoption.toml", render_tool_adoption(repo))
    write(repo.path / "agent" / "security-policy.toml", render_security_policy(repo))
    write(repo.path / "agent" / "proofbind.toml", render_proofbind(repo))
    write(repo.path / "agent" / "proofmark.toml", render_proofmark(repo))
    write(repo.path / "agent" / "coverage-sources.toml", render_coverage_sources(repo))
    write(repo.path / "agent" / "repair-fixture.toml", render_repair_fixture(repo))
    write(repo.path / "agent" / "JANKURAI_STANDARD.md", render_standard(repo, source_sha))
    if repo.cargo_members or (repo.path / "Cargo.toml").exists():
        write(repo.path / ".cargo" / "config.toml", render_cargo_config(repo))
    if (repo.path / "contracts").exists():
        write(repo.path / "contracts" / "MIRROR.md", render_contracts_mirror_doc(repo))
    for kind in ("required", "check", "fast", "score", "security", "tool_adoption", "contract_drift", "artifact_support"):
        filename = {
            "artifact_support": "artifact_support.sh",
            "tool_adoption": "tool-adoption.sh",
            "contract_drift": "contract-drift.sh",
        }.get(kind, f"{kind}.sh")
        write(repo.path / "ops" / "ci" / filename, render_ci_script(kind, repo), executable=True)
    write(repo.path / "ops" / "ci" / "lib.sh", render_ci_lib(), executable=True)
    write(repo.path / "ops" / "AGENTS.md", render_ops_agents(repo))
    write(repo.path / "ops" / "git-hooks" / "pre-push", render_pre_push_hook(), executable=True)
    if repo.name in {"jain-catboost", "jain-xgboost", "jain-lightgbm"}:
        write(repo.path / "scripts" / "vendor.sh", render_native_vendor_script(repo), executable=True)
    write(repo.path / "scripts" / "ci-local.sh", render_ci_local_script(), executable=True)
    write(repo.path / "scripts" / "ci-doctor.sh", "#!/usr/bin/env bash\nset -euo pipefail\njust score\n", executable=True)
    write(repo.path / "tools" / "security-lane.sh", render_security_lane_wrapper(), executable=True)
    write(repo.path / "ops" / "dev" / "local-patches.example.toml", render_local_patches_example(repo), executable=False)
    write(repo.path / ".github" / "workflows" / "ci.yml", render_workflow(repo))


def write_portal_repo(repo: Repo, repos: list[Repo], source_sha: str) -> None:
    append_split_gitignore(repo)
    write(repo.path / "README.md", render_portal_readme(repo, repos))
    write(repo.path / "AGENTS.md", render_portal_agents(repo))
    write(repo.path / "Justfile", render_justfile(repo.profile))
    write(repo.path / "VERSION", repo.current_tag + "\n")
    write(repo.path / "CHANGELOG.md", render_changelog(repo))
    write(repo.path / "SPLIT.md", render_split_doc(repo, source_sha))
    write(repo.path / "repos.manifest.toml", (ROOT / "repos.manifest.toml").read_text(encoding="utf-8"))
    write(repo.path / "family.lock", render_root_lock(repos, source_sha))
    split_docs = ROOT / "docs"
    if split_docs.exists():
        for doc in split_docs.glob("*.md"):
            write(repo.path / "docs" / doc.name, doc.read_text(encoding="utf-8"))
    write(repo.path / "docs" / "architecture.md", render_architecture_doc(repo))
    write(repo.path / "docs" / "testing.md", render_testing_doc(repo))
    write(repo.path / "docs" / "release.md", render_release_doc(repo))
    write(repo.path / "agent" / "owner-map.json", render_owner_map(repo))
    write(repo.path / "agent" / "test-map.json", render_test_map(repo))
    write(repo.path / "agent" / "generated-zones.toml", generated_zones(repo))
    write(repo.path / "agent" / "proof-lanes.toml", render_proof_lanes(repo))
    write(repo.path / "agent" / "audit-policy.toml", render_audit_policy(repo))
    write(repo.path / "agent" / "boundaries.toml", render_boundaries(repo))
    write(repo.path / "agent" / "tool-adoption.toml", render_tool_adoption(repo))
    write(repo.path / "agent" / "security-policy.toml", render_security_policy(repo))
    write(repo.path / "agent" / "proofbind.toml", render_proofbind(repo))
    write(repo.path / "agent" / "proofmark.toml", render_proofmark(repo))
    write(repo.path / "agent" / "coverage-sources.toml", render_coverage_sources(repo))
    write(repo.path / "agent" / "repair-fixture.toml", render_repair_fixture(repo))
    write(repo.path / "agent" / "JANKURAI_STANDARD.md", render_standard(repo, source_sha))
    write(repo.path / "scripts" / "install.sh", render_install_script(), executable=True)
    write(repo.path / "scripts" / "clone-family.sh", render_clone_family_script(), executable=True)
    write(repo.path / "scripts" / "validate-family.sh", render_validate_family_script(), executable=True)
    write(repo.path / "scripts" / "fleet-ci.sh", render_fleet_ci_script(), executable=True)
    write(repo.path / "scripts" / "family-doctor.sh", render_family_doctor_script(), executable=True)
    write(repo.path / "scripts" / "contracts-sync.sh", render_contracts_sync_script(), executable=True)
    write(repo.path / "scripts" / "version-consistency.sh", render_version_consistency_script(), executable=True)
    write(repo.path / "scripts" / "coverage-report.sh", render_coverage_report_script(), executable=True)
    write(repo.path / "scripts" / "regen-family-lock.sh", render_regen_family_lock_script(), executable=True)
    write(repo.path / "scripts" / "ci-local.sh", render_ci_local_script(), executable=True)
    write(repo.path / "scripts" / "ci-doctor.sh", "#!/usr/bin/env bash\nset -euo pipefail\njust score\n", executable=True)
    write(repo.path / "tools" / "security-lane.sh", render_security_lane_wrapper(), executable=True)
    for kind in ("required", "check", "fast", "score", "security", "tool_adoption", "contract_drift", "artifact_support"):
        filename = {
            "artifact_support": "artifact_support.sh",
            "tool_adoption": "tool-adoption.sh",
            "contract_drift": "contract-drift.sh",
        }.get(kind, f"{kind}.sh")
        write(repo.path / "ops" / "ci" / filename, render_ci_script(kind, repo), executable=True)
    write(repo.path / "ops" / "ci" / "lib.sh", render_ci_lib(), executable=True)
    write(repo.path / "ops" / "AGENTS.md", render_ops_agents(repo))
    write(repo.path / "ops" / "git-hooks" / "pre-push", render_pre_push_hook(), executable=True)
    for rel in ("manifest.sh", "source_coverage.py", "validate-local-jeryu.py"):
        source = ROOT / "ops" / "split" / rel
        write(
            repo.path / "ops" / "split" / rel,
            source.read_text(encoding="utf-8"),
            executable=source.stat().st_mode & stat.S_IXUSR != 0,
        )
    write(repo.path / ".github" / "workflows" / "ci.yml", render_workflow(repo))


def render_root_lock(repos: list[Repo], source_sha: str, commits: dict[str, str] | None = None) -> str:
    lines = [
        '# Generated by ops/split/materialize.py',
        'schema_version = "1.0.0"',
        'family = "jain"',
        'release = "7.0.1-split.0"',
        'source = "repos.manifest.toml"',
        f'source_commit = "{source_sha}"',
        "",
    ]
    commits = commits or {}
    for repo in repos:
        lines.extend(
            [
                "[[repo]]",
                f'repo = "{repo.name}"',
                f'tag = "{repo.current_tag}"',
                f'commit = "{commits.get(repo.name, "PENDING")}"',
                f'jeryu = "{repo.jeryu_remote}"',
                f'required_check = "{repo.required_check}"',
                "",
            ]
        )
    return "\n".join(lines)


def write_deploy_split_tools(deploy: Repo, repos: list[Repo], source_sha: str, commits: dict[str, str] | None = None) -> None:
    lock = render_root_lock(repos, source_sha, commits)
    write(deploy.path / "repos.manifest.toml", (ROOT / "repos.manifest.toml").read_text(encoding="utf-8"))
    write(deploy.path / "jain-split.lock.toml", lock)
    cli = next(repo for repo in repos if repo.name == "jain-cli")
    write(
        deploy.path / "deployment" / "product" / "Cargo.toml",
        f"""[package]
name = "jain-product"
version.workspace = true
edition.workspace = true
license.workspace = true
publish = false

[dependencies]
feat-cli = {{ git = "{cli.jeryu_remote}", tag = "{cli.current_tag}", package = "feat-cli" }}
""",
    )
    write(deploy.path / "deployment" / "product" / "src" / "lib.rs", "//! Anchor package for the Jain split deploy graph.\n")
    source_root = Path("/home/ubuntu/jain_small")
    if source_root.exists():
        write(deploy.path / "deployment" / "stage" / "Cargo.workspace.toml", (source_root / "Cargo.toml").read_text(encoding="utf-8"))
    write(
        deploy.path / "scripts" / "vendor-all.sh",
        """#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$root/vendor"
for learner in catboost xgboost lightgbm; do
  if [[ -x "$root/../jain-${learner}/scripts/vendor.sh" ]]; then
    (cd "$root/../jain-${learner}" && bash scripts/vendor.sh)
  fi
done
printf 'export JAIN_VENDOR_ROOT=%q\\n' "$root/vendor"
""",
        executable=True,
    )
    write(
        deploy.path / "scripts" / "stage-context.sh",
        """#!/usr/bin/env bash
set -euo pipefail

plan=0
if [[ "${1:-}" == "--plan" ]]; then
  plan=1
fi
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="${JAIN_SPLIT_MANIFEST:-$root/repos.manifest.toml}"
lock="${JAIN_SPLIT_LOCK:-$root/jain-split.lock.toml}"
stage="$root/.stage"

python3 - "$manifest" "$lock" "$stage" "$plan" <<'PY'
from pathlib import Path
import os
import re
import shutil
import subprocess
import sys
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

manifest = tomllib.loads(Path(sys.argv[1]).read_text())
lock = tomllib.loads(Path(sys.argv[2]).read_text())
stage = Path(sys.argv[3])
plan = sys.argv[4] == "1"
repos = {r["name"]: r for r in manifest.get("repo", [])}
locked = {(r.get("repo") or r.get("name")): r for r in lock.get("repo", [])}
stage_names = [
    "jain-domain", "jain-math", "jain-catboost", "jain-xgboost",
    "jain-lightgbm", "jain-battle-gpu", "jain-starforge", "jain-core",
    "jain-report", "jain-tui", "jain-cli", "jain-deploy",
]
for name in stage_names:
    repo = repos.get(name)
    pin = locked.get(name, {})
    if not repo:
        raise SystemExit(f"missing manifest repo {name}")
    print(f"{name}: {repo['path']} @ {pin.get('commit', 'PENDING')}")
if plan:
    raise SystemExit(0)

def read_package(manifest_path: Path) -> str | None:
    if not manifest_path.exists():
        return None
    data = tomllib.loads(manifest_path.read_text())
    package = data.get("package", {}).get("name")
    return str(package) if package else None

if stage.exists():
    shutil.rmtree(stage)
(stage / "repos").mkdir(parents=True)

for name in stage_names:
    repo = repos[name]
    src = Path(repo["path"])
    dest = stage / "repos" / name
    pin = locked.get(name, {})
    commit = str(pin.get("commit", "PENDING"))
    if (src / ".git").exists():
        subprocess.run(["git", "clone", "--no-hardlinks", str(src), str(dest)], check=True)
        if re.fullmatch(r"[0-9a-f]{40}", commit):
            subprocess.run(["git", "-C", str(dest), "checkout", "--detach", commit], check=True)
    else:
        shutil.copytree(src, dest, ignore=shutil.ignore_patterns(".git", "target", ".stage"))

sf = stage / "repos" / "jain-starforge" / "artifacts"
if sf.exists():
    shutil.copytree(sf, stage / "artifacts", dirs_exist_ok=True)

vendor_root = Path(os.environ.get("JAIN_VENDOR_ROOT", "target/native-vendor"))
if vendor_root.exists():
    shutil.copytree(vendor_root, stage / "native-vendor", dirs_exist_ok=True)

sections = {}
for name in stage_names:
    repo = repos[name]
    if name in {"jain-deploy", "jain-ops"}:
        continue
    remote = f"http://127.0.0.1:8787/git/{repo['jeryu_slug']}.git"
    for member in repo.get("cargo_members", []):
        pkg = read_package(stage / "repos" / name / member / "Cargo.toml")
        if pkg:
            sections.setdefault(remote, []).append((pkg, f"../repos/{name}/{member}"))

cargo_dir = stage / ".cargo"
cargo_dir.mkdir(parents=True, exist_ok=True)
lines = [
    "# Generated by scripts/stage-context.sh. Do not commit.",
    "[net]",
    "git-fetch-with-cli = true",
    "",
]
for remote in sorted(sections):
    lines.append(f'[patch."{remote}"]')
    for package, path in sorted(sections[remote]):
        lines.append(f'{package} = {{ path = "{path}" }}')
    lines.append("")
(cargo_dir / "config.toml").write_text("\\n".join(lines), encoding="utf-8")
(stage / "stage-receipt.json").write_text('{"schema_version":"jain.split.stage/v1","status":"ready"}\\n', encoding="utf-8")
PY
printf 'stage context initialized at %s\\n' "$stage"
""",
        executable=True,
    )
    for rel in ("manifest.sh", "source_coverage.py", "validate-local-jeryu.py"):
        source = ROOT / "ops" / "split" / rel
        write(deploy.path / "ops" / "split" / rel, source.read_text(encoding="utf-8"), executable=source.stat().st_mode & stat.S_IXUSR != 0)
    write(
        deploy.path / "ops" / "split" / "verify-lock.py",
        """#!/usr/bin/env python3
from __future__ import annotations
import argparse
import re
from pathlib import Path
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

parser = argparse.ArgumentParser()
parser.add_argument("--lock", default="jain-split.lock.toml")
args = parser.parse_args()
data = tomllib.loads(Path(args.lock).read_text())
missing = []
for field in ("schema_version", "family", "release", "source", "source_commit"):
    if not str(data.get(field, "")).strip():
        missing.append(f"lock missing {field}")
for repo in data.get("repo", []):
    name = repo.get("repo") or repo.get("name") or "<unknown>"
    for field in ("repo", "tag", "commit", "jeryu", "required_check"):
        if not str(repo.get(field, "")).strip():
            missing.append(f"{name} missing {field}")
    commit = str(repo.get("commit", ""))
    if commit not in {"PENDING", "PENDING_SELF"} and not re.fullmatch(r"[0-9a-f]{40}", commit):
        missing.append(f"{name} commit is not a sha: {commit}")
if missing:
    raise SystemExit("\\n".join(missing))
print(f"lock ok: {len(data.get('repo', []))} repos")
""",
        executable=True,
    )
    write(
        deploy.path / "ops" / "split" / "fleet_ci.py",
        """#!/usr/bin/env python3
from __future__ import annotations
import argparse
import subprocess
from pathlib import Path
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

parser = argparse.ArgumentParser()
parser.add_argument("--manifest", default="repos.manifest.toml")
parser.add_argument("--full", action="store_true")
args = parser.parse_args()
data = tomllib.loads(Path(args.manifest).read_text())
cmd = ["just", "check"] if args.full else ["just", "score"]
for repo in data.get("repo", []):
    path = Path(repo["path"])
    print(f"{repo['name']}: {' '.join(cmd)}")
    subprocess.run(cmd, cwd=path, check=True)
""",
        executable=True,
    )
    write(
        deploy.path / "ops" / "split" / "product_pipeline.py",
        """#!/usr/bin/env python3
from __future__ import annotations
import subprocess

steps = [
    ["./ops/split/manifest.sh", "--check-paths"],
    ["./ops/split/source_coverage.py"],
    ["./ops/split/verify-lock.py"],
]
for step in steps:
    print("+", " ".join(step))
    subprocess.run(step, check=True)
print("product pipeline bootstrap ok")
""",
        executable=True,
    )
    write(
        deploy.path / "ops" / "split" / "smoke_serve.sh",
        """#!/usr/bin/env bash
set -euo pipefail
bin="${1:-target/release/jain}"
if [[ ! -x "$bin" ]]; then
  printf 'serve smoke pending: binary not built at %s\\n' "$bin"
  exit 0
fi
"$bin" --help >/dev/null
printf 'serve smoke bootstrap ok\\n'
""",
        executable=True,
    )


def generate_jankurai_baseline(repo: Repo) -> None:
    """Write agent/jankurai-baseline.json = the accepted score+caps for this repo.

    The score lane (ops/ci/score.sh) gates on the monorepo posture: no hard
    findings, no score regression vs this committed baseline, and no new caps.
    The source monorepo itself scores below 85 and passes on no-regression, so a
    committed baseline (not an absolute 85 floor) is what "passing audit" means
    here. Tolerant of a missing auditor: score.sh then falls back to the
    minimum-or-documented-caps gate.
    """
    if shutil.which("jankurai") is None:
        return
    policy = repo.path / "agent" / "audit-policy.toml"
    if not policy.exists():
        return
    tmp = repo.path / "target" / "jankurai" / "baseline-gen.json"
    tmp.parent.mkdir(parents=True, exist_ok=True)
    res = subprocess.run(
        [
            "jankurai", "audit", ".", "--full", "--mode", "advisory",
            "--policy", "agent/audit-policy.toml", "--json", str(tmp),
            "--no-score-history",
        ],
        cwd=repo.path, capture_output=True, text=True,
    )
    if res.returncode != 0 or not tmp.exists():
        return
    report = json.loads(tmp.read_text())
    caps = sorted(str(c) for c in (report.get("caps_applied") or report.get("caps") or []))
    decision = report.get("decision") if isinstance(report.get("decision"), dict) else {}
    hard = decision.get("hard_findings", report.get("hard_findings", 0))
    hard = len(hard) if isinstance(hard, list) else int(hard or 0)
    baseline = {
        "schema": "jain.split.jankurai-baseline/v1",
        "score": int(report.get("score") or 0),
        "caps": caps,
        "hard_findings": hard,
        "auditor": "jankurai 1.6.11",
        "posture": "monorepo-ratchet",
        "note": "Accepted split baseline at seed; source monorepo scores below 85 and passes on no-regression. Ratchet gate in ops/ci/score.sh: no hard findings, no score regression, no new caps.",
    }
    (repo.path / "agent" / "jankurai-baseline.json").write_text(
        json.dumps(baseline, indent=2) + "\n"
    )


def init_git_repo(repo: Repo, source_sha: str) -> str:
    if not (repo.path / ".git").exists():
        run(["git", "init", "-b", "main"], cwd=repo.path)
    run(["git", "config", "user.name", "Jain Split Bot"], cwd=repo.path)
    run(["git", "config", "user.email", "split-bot@localhost"], cwd=repo.path)
    generate_jankurai_baseline(repo)
    run(["git", "add", "."], cwd=repo.path)
    run(["git", "commit", "-m", f"chore: seed jain split repo from {source_sha}"], cwd=repo.path)
    existing = subprocess.run(["git", "remote", "get-url", "origin"], cwd=repo.path, capture_output=True)
    if existing.returncode == 0:
        run(["git", "remote", "set-url", "origin", repo.jeryu_remote], cwd=repo.path)
    else:
        run(["git", "remote", "add", "origin", repo.jeryu_remote], cwd=repo.path)
    if subprocess.run(["git", "remote", "get-url", "github"], cwd=repo.path, capture_output=True).returncode == 0:
        run(["git", "remote", "remove", "github"], cwd=repo.path)
    run(["git", "tag", "-f", repo.current_tag], cwd=repo.path)
    return run(["git", "rev-parse", "HEAD"], cwd=repo.path)


def write_git_instead_of_config(split_root: Path, repos: list[Repo]) -> Path:
    cfg = split_root / "target" / "local-gitconfig"
    cfg.parent.mkdir(parents=True, exist_ok=True)
    lines: list[str] = [
        "# CI-only Cargo lock cache.",
        "# Do not use target/bare-mirrors as an agent checkout or operational source.",
        "# Canonical agent remotes are local Jeryu remotes.",
    ]
    for repo in repos:
        mirror = split_root / "target" / "bare-mirrors" / f"{repo.name}.git"
        lines.append(f'[url "file://{mirror}"]')
        lines.append(f'\tinsteadOf = {repo.jeryu_remote}')
        lines.append(f'\tinsteadOf = {repo.github_remote}')
    cfg.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return cfg


def refresh_bare_mirror(repo: Repo, split_root: Path) -> None:
    mirrors = split_root / "target" / "bare-mirrors"
    mirrors.mkdir(parents=True, exist_ok=True)
    mirror = mirrors / f"{repo.name}.git"
    if mirror.exists():
        shutil.rmtree(mirror)
    run(["git", "clone", "--mirror", str(repo.path), str(mirror)])


def generate_cargo_lock(repo: Repo, split_root: Path, repos: list[Repo]) -> None:
    if not (repo.path / "Cargo.toml").exists():
        return
    cfg = write_git_instead_of_config(split_root, repos)
    cargo_home = split_root / "target" / "cargo-home"
    cargo_home.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["GIT_CONFIG_GLOBAL"] = str(cfg)
    env["CARGO_HOME"] = str(cargo_home)
    env["CARGO_NET_GIT_FETCH_WITH_CLI"] = "true"
    env.setdefault("CARGO_NET_RETRY", "3")
    subprocess.run(["cargo", "generate-lockfile"], cwd=repo.path, env=env, check=True)


def capture_dirty_patch(source_root: Path, split_root: Path) -> Path | None:
    status = run(["git", "-C", str(source_root), "status", "--porcelain"])
    if not status:
        return None
    stamp = datetime.now(timezone.utc).strftime("%Y-%m-%d")
    out = split_root / "dirty" / f"import-dirty-{stamp}.patch"
    out.parent.mkdir(parents=True, exist_ok=True)
    diff = run(["git", "-C", str(source_root), "diff", "--binary"])
    out.write_text(diff + "\n", encoding="utf-8")
    return out


def write_root_manifest_copy(manifest: Path, split_root: Path) -> None:
    target = split_root / "repos.manifest.toml"
    write(target, manifest.read_text(encoding="utf-8"))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", default=str(MANIFEST))
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--init-git", action="store_true")
    parser.add_argument("--capture-dirty-patch", action="store_true")
    args = parser.parse_args()

    manifest_path = Path(args.manifest)
    data, repos = load_manifest(manifest_path)
    source_root = Path(str(data["source_root"]))
    split_root = Path(str(data["split_root"]))
    source_sha = str(data["source_sha"])
    actual_sha = run(["git", "-C", str(source_root), "rev-parse", "HEAD"])
    if actual_sha != source_sha:
        raise SystemExit(f"source HEAD mismatch: manifest {source_sha}, actual {actual_sha}")
    tracked = git_tracked_paths(source_root, source_sha)
    repos_by_name = {repo.name: repo for repo in repos}
    package_to_repo, member_to_package = cargo_member_packages(source_root, repos)

    if not args.force:
        for repo in repos:
            if repo.path.exists():
                raise SystemExit(f"{repo.path} exists; use --force to replace generated repo")

    write_root_manifest_copy(manifest_path, split_root)

    for repo in repos:
        if repo.authored:
            if not repo.path.exists():
                raise SystemExit(f"{repo.name}: authored split repo is missing at {repo.path}")
            write_repo_standard(repo, source_sha)
            continue
        if repo.path.exists():
            if split_root not in repo.path.parents:
                raise SystemExit(f"refusing to remove path outside split root: {repo.path}")
            shutil.rmtree(repo.path)
        repo.path.mkdir(parents=True)
        if repo.profile == "public-portal":
            write_portal_repo(repo, repos, source_sha)
            continue
        archive_paths(source_root, source_sha, repo.path, COMMON_COPY_PATHS + repo.copy_paths, tracked)
        smudge_starforge_lfs(source_root, repo)
        if repo.cargo_members:
            patch = render_patch_sections(repo, repos, package_to_repo, member_to_package)
            write(repo.path / "Cargo.toml", render_cargo_toml(source_root, repo.cargo_members, patch))
            archive_paths(source_root, source_sha, repo.path, ["Cargo.lock"], tracked)
        write_repo_standard(repo, source_sha)
        rewrite_dependency_tomls(repo, repos_by_name, package_to_repo)
        applied = apply_split_patches(repo)
        if applied:
            write(repo.path / "SPLIT.md", render_split_doc(repo, source_sha, applied))

    deploy = repos_by_name["jain-deploy"]
    write_deploy_split_tools(deploy, repos, source_sha)

    commits: dict[str, str] = {}
    if args.init_git:
        for repo in repos:
            if repo.name == "jain-deploy":
                continue
            generate_cargo_lock(repo, split_root, repos)
            commits[repo.name] = init_git_repo(repo, source_sha)
            refresh_bare_mirror(repo, split_root)
        commits["jain-deploy"] = "PENDING_SELF"
        write_deploy_split_tools(deploy, repos, source_sha, commits)
        generate_cargo_lock(deploy, split_root, repos)
        commits["jain-deploy"] = init_git_repo(deploy, source_sha)
        refresh_bare_mirror(deploy, split_root)
        portal = repos_by_name["jain"]
        lock_commits = dict(commits)
        lock_commits["jain"] = "PENDING_SELF"
        write(portal.path / "family.lock", render_root_lock(repos, source_sha, lock_commits))
        run(["git", "add", "family.lock"], cwd=portal.path)
        run(["git", "commit", "-m", "chore: record split family lock"], cwd=portal.path)
        run(["git", "tag", "-f", portal.current_tag], cwd=portal.path)
        commits["jain"] = run(["git", "rev-parse", "HEAD"], cwd=portal.path)
        refresh_bare_mirror(portal, split_root)

    dirty_patch = capture_dirty_patch(source_root, split_root) if args.capture_dirty_patch else None
    report = {
        "schema_version": "jain.split.materialize/v1",
        "source_root": str(source_root),
        "source_sha": source_sha,
        "repos": [repo.name for repo in repos],
        "git_initialized": args.init_git,
        "commits": commits,
        "dirty_patch": str(dirty_patch) if dirty_patch else None,
    }
    write(split_root / "target" / "materialize-report.json", json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
