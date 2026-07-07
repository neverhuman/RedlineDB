#!/usr/bin/env bash
set -euo pipefail

manifest="repos.manifest.toml"
base="${JERYU_BASE:-http://127.0.0.1:8787}"
family="${JERYU_REPO_FAMILY:-}"
check_only=0

usage() {
  printf 'usage: %s [--manifest PATH] [--base URL] [--family NAME] [--check-only]\n' "$0" >&2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --manifest)
      shift
      manifest="${1:-}"
      ;;
    --base)
      shift
      base="${1:-}"
      ;;
    --family)
      shift
      family="${1:-}"
      ;;
    --check-only)
      check_only=1
      ;;
    *)
      usage
      exit 2
      ;;
  esac
  shift
done

[[ -n "$manifest" ]] || { usage; exit 2; }
[[ -r "$manifest" ]] || { printf 'manifest not readable: %s\n' "$manifest" >&2; exit 1; }
[[ -n "$base" ]] || { printf 'base URL must not be empty\n' >&2; exit 1; }

mapfile -t rows < <(
  python3 - "$manifest" "$family" <<'PY'
import sys
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

manifest_path, override_family = sys.argv[1], sys.argv[2]
with open(manifest_path, "rb") as fh:
    data = tomllib.load(fh)

family = override_family or data.get("repo_family") or "jain-split"
if not isinstance(family, str) or not family.strip():
    raise SystemExit("repo family must be a non-empty string")

for repo in data.get("repo", []):
    slug = repo.get("jeryu_slug")
    if not isinstance(slug, str) or "/" not in slug:
        raise SystemExit(f"invalid jeryu_slug for {repo.get('name', '<unknown>')}: {slug!r}")
    owner, name = slug.split("/", 1)
    print("|".join([family.strip(), owner, name]))
PY
)

[[ "${#rows[@]}" -gt 0 ]] || { printf 'manifest has no repo entries\n' >&2; exit 1; }
family="${rows[0]%%|*}"

if [[ "$check_only" != "1" ]]; then
  for row in "${rows[@]}"; do
    IFS='|' read -r row_family owner name <<<"$row"
    body="$(python3 -c 'import json,sys; print(json.dumps({"family": sys.argv[1]}))' "$row_family")"
    curl -fsS -X PATCH "$base/api/v1/repos/${owner}%2F${name}" \
      -H 'content-type: application/json' \
      -d "$body" >/dev/null
    printf 'registered %s/%s -> %s\n' "$owner" "$name" "$row_family"
  done
fi

repos_json="$(curl -fsS "$base/api/v1/repos?host=jeryu")"
REPOS_JSON="$repos_json" python3 - "$family" "${rows[@]}" <<'PY'
import json
import os
import sys

family = sys.argv[1]
expected = [tuple(row.split("|", 2)[1:]) for row in sys.argv[2:]]
payload = json.loads(os.environ["REPOS_JSON"])
by_slug = {
    (repo["id"]["owner"], repo["id"]["name"]): repo
    for repo in payload.get("repositories", [])
}
missing = []
wrong = []
for owner, name in expected:
    repo = by_slug.get((owner, name))
    if repo is None:
        missing.append(f"{owner}/{name}")
    elif repo.get("family") != family:
        wrong.append(f"{owner}/{name}={repo.get('family')!r}")

facets = set(payload.get("facets", {}).get("families", []))
if family not in facets:
    wrong.append(f"facets missing {family!r}")

if missing or wrong:
    if missing:
        print("missing repos: " + ", ".join(missing), file=sys.stderr)
    if wrong:
        print("family mismatch: " + ", ".join(wrong), file=sys.stderr)
    raise SystemExit(1)

print(f"verified {len(expected)} repos in family {family}")
PY
