#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_BASE = "http://127.0.0.1:8787"


def load_manifest(path: Path) -> list[dict[str, Any]]:
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    repos = data.get("repo", [])
    if not isinstance(repos, list) or not repos:
        raise SystemExit(f"{path} has no [[repo]] entries")
    return [repo for repo in repos if isinstance(repo, dict)]


def load_family(path: Path) -> str:
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    family = data.get("repo_family") or data.get("family") or "jain-split"
    if not isinstance(family, str) or not family.strip():
        raise SystemExit(f"{path} has no valid repo_family")
    return family.strip()


def expected_remote(base: str, repo: dict[str, Any]) -> str:
    slug = str(repo.get("jeryu_slug") or f"jeryu/{repo['name']}")
    return f"{base.rstrip('/')}/git/{slug}.git"


def run(cmd: list[str], *, quiet: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        check=False,
        text=True,
        stdout=subprocess.PIPE if quiet else None,
        stderr=subprocess.PIPE if quiet else None,
    )


def token_from_git_credentials(base: str) -> str | None:
    parsed = urllib.parse.urlparse(base)
    if not parsed.scheme or not parsed.netloc:
        return None
    result = subprocess.run(
        ["git", "credential", "fill"],
        input=f"protocol={parsed.scheme}\nhost={parsed.netloc}\n\n",
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if result.returncode != 0:
        return None
    for line in result.stdout.splitlines():
        if line.startswith("password="):
            return line.split("=", 1)[1].strip()
    return None


def read_token(base: str, token_file: Path) -> str | None:
    env_token = os.environ.get("JERYU_MERGE_TOKEN")
    if env_token:
        return env_token.strip()
    credential_token = token_from_git_credentials(base)
    if credential_token:
        return credential_token
    if token_file.is_file():
        return token_file.read_text(encoding="utf-8").strip()
    return None


def http_get(base: str, path: str, token: str | None) -> tuple[int, str]:
    headers = {"accept": "application/json"}
    if token:
        headers["authorization"] = f"Bearer {token}"
    request = urllib.request.Request(f"{base.rstrip('/')}{path}", headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=5) as response:
            return response.status, response.read().decode("utf-8", errors="replace")
    except urllib.error.HTTPError as exc:
        return exc.code, exc.read().decode("utf-8", errors="replace")
    except OSError as exc:
        return 0, str(exc)


def http_json(
    method: str,
    base: str,
    path: str,
    token: str,
    body: dict[str, Any],
) -> tuple[int, str]:
    request = urllib.request.Request(
        f"{base.rstrip('/')}{path}",
        data=json.dumps(body, separators=(",", ":")).encode("utf-8"),
        headers={
            "accept": "application/json",
            "authorization": f"Bearer {token}",
            "content-type": "application/json",
        },
        method=method,
    )
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            return response.status, response.read().decode("utf-8", errors="replace")
    except urllib.error.HTTPError as exc:
        return exc.code, exc.read().decode("utf-8", errors="replace")
    except OSError as exc:
        return 0, str(exc)


def git_remote_names(repo_path: Path) -> list[str]:
    result = run(["git", "-C", str(repo_path), "remote"])
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "git remote failed")
    return [line.strip() for line in result.stdout.splitlines() if line.strip()]


def git_remote_urls(repo_path: Path, name: str) -> list[str]:
    result = run(["git", "-C", str(repo_path), "remote", "get-url", "--all", name])
    if result.returncode != 0:
        return []
    return [line.strip() for line in result.stdout.splitlines() if line.strip()]


def fix_remote(repo_path: Path, remote: str) -> None:
    names = git_remote_names(repo_path)
    if "origin" in names:
        result = run(["git", "-C", str(repo_path), "remote", "set-url", "origin", remote])
    else:
        result = run(["git", "-C", str(repo_path), "remote", "add", "origin", remote])
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "failed to set origin")
    for name in names:
        if name == "origin":
            continue
        result = run(["git", "-C", str(repo_path), "remote", "remove", name])
        if result.returncode != 0:
            raise RuntimeError(result.stderr.strip() or f"failed to remove remote {name}")


def check_or_fix_remotes(
    repos: list[dict[str, Any]],
    base: str,
    *,
    fix: bool,
    issues: list[str],
) -> None:
    for repo in repos:
        name = str(repo["name"])
        repo_path = Path(str(repo["path"]))
        remote = expected_remote(base, repo)
        if not (repo_path / ".git").exists():
            issues.append(f"{name}: missing git checkout at {repo_path}")
            continue
        try:
            if fix:
                fix_remote(repo_path, remote)
            names = git_remote_names(repo_path)
            origin_urls = git_remote_urls(repo_path, "origin")
        except RuntimeError as exc:
            issues.append(f"{name}: {exc}")
            continue
        if origin_urls != [remote]:
            issues.append(f"{name}: origin must be exactly {remote}")
        extra = [remote_name for remote_name in names if remote_name != "origin"]
        if extra:
            issues.append(f"{name}: remove non-canonical remotes: {', '.join(extra)}")


def register_family(
    base: str,
    token: str,
    repos: list[dict[str, Any]],
    family: str,
    issues: list[str],
) -> None:
    for repo in repos:
        slug = str(repo.get("jeryu_slug") or f"jeryu/{repo['name']}")
        status, body = http_json(
            "PATCH",
            base,
            f"/api/v1/repos/{urllib.parse.quote(slug, safe='')}",
            token,
            {"family": family},
        )
        if status not in {200, 204}:
            issues.append(f"{slug}: could not register family {family} (status {status}: {body[:120]})")


def check_forge_api(
    base: str,
    token: str | None,
    repos: list[dict[str, Any]],
    family: str,
    *,
    fix_family: bool,
    issues: list[str],
) -> None:
    status, body = http_get(base, "/health", None)
    if status != 200:
        status, body = http_get(base, "/api/v1/version", None)
    if status != 200:
        issues.append(f"local Jeryu is not reachable at {base} (status {status}: {body[:120]})")
        return

    if not token:
        issues.append("local Jeryu API credential is unavailable; configure Git credentials for the loopback host")
        return

    if fix_family:
        register_family(base, token, repos, family, issues)

    status, body = http_get(base, "/.jeryu/capabilities", token)
    if status not in {200, 404}:
        issues.append(f"local Jeryu auth failed for capabilities (status {status})")

    status, body = http_get(base, "/api/v1/repos?host=jeryu", token)
    if status != 200:
        issues.append(f"local Jeryu repo list failed (status {status})")
        return

    try:
        payload = json.loads(body)
    except json.JSONDecodeError as exc:
        issues.append(f"local Jeryu repo list returned invalid JSON: {exc}")
        return

    by_slug: dict[str, dict[str, Any]] = {}
    for repo in payload.get("repositories", []):
        if not isinstance(repo, dict):
            continue
        ident = repo.get("id", {})
        slug = f"{ident.get('owner')}/{ident.get('name')}"
        by_slug[slug] = repo
    expected = {str(repo.get("jeryu_slug") or f"jeryu/{repo['name']}") for repo in repos}
    missing = sorted(expected - set(by_slug))
    if missing:
        issues.append("local Jeryu is missing repos: " + ", ".join(missing))
    wrong_family = []
    for slug in sorted(expected & set(by_slug)):
        actual_family = by_slug[slug].get("family")
        if actual_family != family:
            wrong_family.append(f"{slug}={actual_family!r}")
    if wrong_family:
        issues.append("local Jeryu family mismatch: " + ", ".join(wrong_family))


def setup_auth(base: str, token_file: Path, issues: list[str]) -> None:
    if not token_file.is_file():
        issues.append("local Jeryu token file is missing; cannot run optional gh-setup")
        return
    result = run(
        [
            "jeryu",
            "gh-setup",
            "--host",
            base,
            "--token-file",
            str(token_file),
        ]
    )
    if result.returncode != 0:
        issues.append(f"jeryu gh-setup failed with exit code {result.returncode}")


def validate_policy(manifest: Path, issues: list[str]) -> None:
    result = run(
        [
            sys.executable,
            str(ROOT / "ops" / "split" / "validate-local-jeryu.py"),
            "--manifest",
            str(manifest),
        ]
    )
    if result.returncode != 0:
        output = (result.stderr or result.stdout).strip()
        issues.append("local Jeryu policy validator failed:\n" + output)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Prepare and verify local Jeryu access for every Jain split repo."
    )
    parser.add_argument("--manifest", default=str(ROOT / "repos.manifest.toml"))
    parser.add_argument("--base", default=os.environ.get("JERYU_BASE", DEFAULT_BASE))
    parser.add_argument(
        "--token-file",
        default=os.environ.get("JERYU_MERGE_TOKEN_FILE", "~/.jeryu/secrets/merge-token"),
        help="optional local token path fallback; the token is never printed",
    )
    parser.add_argument("--setup-auth", action="store_true", help="run jeryu gh-setup")
    parser.add_argument("--fix-remotes", action="store_true", help="canonicalize repo remotes")
    parser.add_argument("--register-family", action="store_true", help="set expected local Jeryu family metadata")
    parser.add_argument(
        "--skip-policy",
        action="store_true",
        help="skip the full local-source policy validator",
    )
    args = parser.parse_args()

    manifest = Path(args.manifest).resolve()
    base = str(args.base).rstrip("/")
    token_file = Path(str(args.token_file)).expanduser()
    repos = load_manifest(manifest)
    family = load_family(manifest)
    token = read_token(base, token_file)
    issues: list[str] = []

    print(f"[jeryu-doctor] workspace: {manifest.parent}")
    print(f"[jeryu-doctor] forge: {base}")
    print("[jeryu-doctor] credential source: Git credential helper or local environment")

    if args.setup_auth:
        setup_auth(base, token_file, issues)
        token = read_token(base, token_file)

    check_forge_api(base, token, repos, family, fix_family=args.register_family, issues=issues)
    check_or_fix_remotes(repos, base, fix=args.fix_remotes, issues=issues)
    if not args.skip_policy:
        validate_policy(manifest, issues)

    if issues:
        print("[jeryu-doctor] issues:", file=sys.stderr)
        for issue in issues:
            print(f"- {issue}", file=sys.stderr)
        if not args.fix_remotes or not args.setup_auth:
            print(
                "[jeryu-doctor] repair: run `just jeryu-ready` from jain-split-ops",
                file=sys.stderr,
            )
        return 1

    print(f"[jeryu-doctor] ok: {len(repos)} repos use local Jeryu")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
