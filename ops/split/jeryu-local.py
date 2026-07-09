#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
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
DEFAULT_OWNER = "jeryu"


def token_from_git_credentials(base: str) -> str | None:
    parsed = urllib.parse.urlparse(base)
    if not parsed.scheme or not parsed.netloc:
        return None
    import subprocess

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


def read_token(base: str, token_file: Path) -> str:
    env_token = os.environ.get("JERYU_MERGE_TOKEN")
    if env_token:
        return env_token.strip()
    credential_token = token_from_git_credentials(base)
    if credential_token:
        return credential_token
    if token_file.is_file():
        return token_file.read_text(encoding="utf-8").strip()
    raise SystemExit("local Jeryu API credential is unavailable; configure Git credentials for the loopback host")


def manifest_slugs(path: Path) -> set[str]:
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    slugs: set[str] = set()
    for repo in data.get("repo", []):
        if not isinstance(repo, dict):
            continue
        slug = repo.get("jeryu_slug") or f"jeryu/{repo.get('name')}"
        if isinstance(slug, str):
            slugs.add(slug)
    return slugs


def split_repo(value: str, default_owner: str) -> tuple[str, str]:
    if "/" in value:
        owner, repo = value.split("/", 1)
        return owner, repo
    return default_owner, value


def request_json(
    method: str,
    base: str,
    path: str,
    token: str,
    body: dict[str, Any] | None = None,
) -> Any:
    data = None
    headers = {
        "accept": "application/json",
        "authorization": f"Bearer {token}",
    }
    if body is not None:
        data = json.dumps(body, separators=(",", ":")).encode("utf-8")
        headers["content-type"] = "application/json"
    request = urllib.request.Request(
        f"{base.rstrip('/')}{path}",
        data=data,
        headers=headers,
        method=method,
    )
    try:
        with urllib.request.urlopen(request, timeout=15) as response:
            raw = response.read().decode("utf-8", errors="replace")
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace").strip()
        raise SystemExit(f"local Jeryu {method} {path} failed: HTTP {exc.code} {detail}") from exc
    if not raw.strip():
        return {}
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        return raw


def filter_payload(payload: Any, repo_filter: set[str] | None) -> Any:
    if repo_filter is None or not isinstance(payload, dict) or "repositories" not in payload:
        return payload
    filtered = []
    for repo in payload["repositories"]:
        ident = repo.get("id", {})
        slug = f"{ident.get('owner')}/{ident.get('name')}"
        if slug in repo_filter:
            filtered.append(repo)
    copy = dict(payload)
    copy["repositories"] = filtered
    copy["total"] = len(filtered)
    return copy


def print_payload(payload: Any, *, json_out: bool, repo_filter: set[str] | None = None) -> None:
    payload = filter_payload(payload, repo_filter)
    if json_out:
        print(json.dumps(payload, indent=2, sort_keys=True))
        return
    if isinstance(payload, dict) and "repositories" in payload:
        for repo in payload["repositories"]:
            ident = repo.get("id", {})
            slug = f"{ident.get('owner')}/{ident.get('name')}"
            if repo_filter is not None and slug not in repo_filter:
                continue
            family = repo.get("family") or "-"
            print(f"{slug} {family}")
        return
    if isinstance(payload, list):
        for item in payload:
            if isinstance(item, dict):
                number = item.get("number") or item.get("id") or "?"
                title = item.get("title") or item.get("state") or json.dumps(item, sort_keys=True)
                print(f"{number} {title}")
            else:
                print(item)
        return
    print(json.dumps(payload, indent=2, sort_keys=True))


def main() -> int:
    parser = argparse.ArgumentParser(description="Jain local-Jeryu REST helper.")
    parser.add_argument("--base", default=os.environ.get("JERYU_BASE", DEFAULT_BASE))
    parser.add_argument("--owner", default=DEFAULT_OWNER)
    parser.add_argument(
        "--token-file",
        default=os.environ.get("JERYU_MERGE_TOKEN_FILE", "~/.jeryu/secrets/merge-token"),
        help="optional token path fallback; the token is never printed",
    )
    parser.add_argument("--json", action="store_true")
    sub = parser.add_subparsers(dest="command", required=True)

    repo_list = sub.add_parser("repo-list", help="list local Jeryu repos from the Jain manifest")
    repo_list.add_argument("--all", action="store_true", help="show every local Jeryu repo")
    repo_list.add_argument("--manifest", default=str(ROOT / "repos.manifest.toml"))
    repo_list.add_argument("--json", action="store_true", default=argparse.SUPPRESS)

    pr_list = sub.add_parser("pr-list", help="list pull requests")
    pr_list.add_argument("--repo", required=True)
    pr_list.add_argument("--state", default="open")
    pr_list.add_argument("--json", action="store_true", default=argparse.SUPPRESS)

    pr_open = sub.add_parser("pr-open", help="open a pull request")
    pr_open.add_argument("--repo", required=True)
    pr_open.add_argument("--head", required=True)
    pr_open.add_argument("--title", required=True)
    pr_open.add_argument("--base", default="main", dest="base_branch")
    pr_open.add_argument("--body", default="")
    pr_open.add_argument("--draft", action="store_true")
    pr_open.add_argument("--actor", default="codex")
    pr_open.add_argument("--json", action="store_true", default=argparse.SUPPRESS)

    pr_merge = sub.add_parser("pr-merge", help="merge a pull request")
    pr_merge.add_argument("--repo", required=True)
    pr_merge.add_argument("--number", required=True)
    pr_merge.add_argument("--json", action="store_true", default=argparse.SUPPRESS)

    checks = sub.add_parser("checks", help="list check runs for a commit")
    checks.add_argument("--repo", required=True)
    checks.add_argument("--sha", required=True)
    checks.add_argument("--json", action="store_true", default=argparse.SUPPRESS)

    args = parser.parse_args()
    token = read_token(args.base, Path(args.token_file).expanduser())
    repo_filter: set[str] | None = None

    if args.command == "repo-list":
        payload = request_json("GET", args.base, "/api/v1/repos?host=jeryu", token)
        if not args.all:
            repo_filter = manifest_slugs(Path(args.manifest))
    else:
        owner, repo = split_repo(args.repo, args.owner)
        repo_path = f"/repos/{urllib.parse.quote(owner)}/{urllib.parse.quote(repo)}"
        if args.command == "pr-list":
            query = urllib.parse.urlencode({"state": args.state})
            payload = request_json("GET", args.base, f"{repo_path}/pulls?{query}", token)
        elif args.command == "pr-open":
            payload = request_json(
                "POST",
                args.base,
                f"{repo_path}/pulls",
                token,
                {
                    "title": args.title,
                    "head": args.head,
                    "base": args.base_branch,
                    "body": args.body,
                    "draft": args.draft,
                    "actor": args.actor,
                },
            )
        elif args.command == "pr-merge":
            payload = request_json("PUT", args.base, f"{repo_path}/pulls/{args.number}/merge", token, {})
        elif args.command == "checks":
            payload = request_json("GET", args.base, f"{repo_path}/commits/{args.sha}/check-runs", token)
        else:  # pragma: no cover
            raise AssertionError(args.command)

    print_payload(payload, json_out=args.json, repo_filter=repo_filter)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
