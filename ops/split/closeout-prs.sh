#!/usr/bin/env bash
# Create closeout PRs for Jain split repos after local validation.
set -uo pipefail

base="${JERYU_BASE:-http://127.0.0.1:8787}"
split="${JAIN_SPLIT_ROOT:-/home/ubuntu/jain-split}"          # family root (sibling repos + target/)
ops_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)" # this control-plane repo (jain-split-ops)
manifest="${JAIN_SPLIT_MANIFEST:-${ops_root}/repos.manifest.toml}"
branch="${JAIN_CLOSEOUT_BRANCH:-closeout-v7-split}"
actor="${JAIN_CLOSEOUT_ACTOR:-split-bot}"
fail=0

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
    slug = repo["jeryu_slug"]
    owner, name = slug.split("/", 1)
    print("|".join([repo["name"], repo["path"], owner, name, repo["required_check"]]))
PY
)

for row in "${rows[@]}"; do
  IFS='|' read -r repo path owner name required <<<"$row"
  printf '===================== %s =====================\n' "$repo"
  if [[ ! -d "$path/.git" ]]; then
    printf '[%s] missing git checkout: %s\n' "$repo" "$path" >&2
    fail=1
    continue
  fi
  git -C "$path" checkout -q main || { fail=1; continue; }
  git -C "$path" pull -q --ff-only origin main 2>/dev/null || true

  changelog="${path}/CHANGELOG.md"
  if [[ -f "$changelog" ]]; then
    python3 - "$changelog" "$repo" <<'PY'
from pathlib import Path
import sys
path = Path(sys.argv[1])
repo = sys.argv[2]
note = "- Jain split baseline validated through local forge closeout.\n"
text = path.read_text()
if note not in text:
    path.write_text(text.replace("\n", "\n\n## Unreleased\n\n" + note, 1) if text.startswith("# ") else note + text)
PY
  fi

  git -C "$path" checkout -q -B "$branch"
  git -C "$path" add -A
  if git -C "$path" diff --cached --quiet; then
    printf '[%s] nothing to commit\n' "$repo"
    git -C "$path" checkout -q main
    continue
  fi
  git -C "$path" -c user.name="Jain Split Bot" -c user.email="split-bot@localhost" \
    commit -q -m "chore(split): close out v7 split baseline [skip-version]"
  sha="$(git -C "$path" rev-parse HEAD)"
  git -C "$path" push -q origin "$branch" 2>/dev/null || git -C "$path" push -q -f origin "$branch"
  git -C "$path" checkout -q main

  pr="$(curl -fsS -X POST "$base/repos/${owner}/${name}/pulls" -H 'content-type: application/json' \
    -d "{\"title\":\"Jain split closeout: ${repo}\",\"head\":\"${branch}\",\"base\":\"main\",\"actor\":\"${actor}\"}" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["number"])')" || { printf '[%s] PR open failed\n' "$repo"; fail=1; continue; }

  if ! JAIN_SPLIT_ROOT="$split" bash "${ops_root}/ops/ci/split-host-ci.sh" "$owner" "$name" "$sha" "$path" "$required"; then
    printf '[%s] required CI failed\n' "$repo"
    fail=1
    continue
  fi
  code="$(curl -s -X PUT "$base/repos/${owner}/${name}/pulls/${pr}/merge" -H 'content-type: application/json' -d '{}' -o "/tmp/jain-closeout-${repo}.json" -w '%{http_code}')"
  [[ "$code" == "200" ]] || { printf '[%s] merge failed (%s)\n' "$repo" "$code"; fail=1; continue; }
  printf '[%s] merged closeout PR #%s\n' "$repo" "$pr"
done

exit "$fail"
