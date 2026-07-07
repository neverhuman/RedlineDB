#!/usr/bin/env bash
# Open and merge local-forge rollout PRs for Jain split repos.
set -uo pipefail

base="${JERYU_BASE:-http://127.0.0.1:8787}"
split="${JAIN_SPLIT_ROOT:-/home/ubuntu/jain-split}"          # family root (sibling repos + target/)
ops_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)" # this control-plane repo (jain-split-ops)
manifest="${JAIN_SPLIT_MANIFEST:-${ops_root}/repos.manifest.toml}"
branch="${JAIN_ROLLOUT_BRANCH:-split/v7-rollout}"
actor="${JAIN_ROLLOUT_ACTOR:-split-bot}"
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
    if repo.get("name") == "jain":
        continue
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
  sha="$(git -C "$path" rev-parse HEAD)" || { fail=1; continue; }

  if curl -fsS -X PUT "$base/repos/${owner}/${name}/branches/main/protection" \
    -H 'content-type: application/json' \
    -d "{\"required_status_checks\":[\"${required}\"],\"required_approving_review_count\":0,\"required_linear_history\":true,\"enforce_admins\":true}" >/dev/null; then
    printf '[%s] protection: %s\n' "$repo" "$required"
  fi

  git -C "$path" push origin "HEAD:refs/heads/${branch}" >/dev/null 2>&1 || true

  pr="$(curl -fsS -X POST "$base/repos/${owner}/${name}/pulls" -H 'content-type: application/json' \
    -d "{\"title\":\"Jain split rollout: ${repo}\",\"head\":\"${branch}\",\"base\":\"main\",\"actor\":\"${actor}\"}" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["number"])')" || { printf '[%s] PR open failed\n' "$repo"; fail=1; continue; }

  if ! JAIN_SPLIT_ROOT="$split" bash "${ops_root}/ops/ci/split-host-ci.sh" "$owner" "$name" "$sha" "$path" "$required"; then
    printf '[%s] required CI failed\n' "$repo"
    fail=1
    continue
  fi

  code="$(curl -s -X PUT "$base/repos/${owner}/${name}/pulls/${pr}/merge" -H 'content-type: application/json' -d '{}' -o "/tmp/jain-rollout-${repo}.json" -w '%{http_code}')"
  if [[ "$code" != "200" ]]; then
    printf '[%s] merge failed (%s): %s\n' "$repo" "$code" "$(head -c 200 "/tmp/jain-rollout-${repo}.json")"
    fail=1
    continue
  fi
  printf '[%s] merged rollout PR #%s at %s\n' "$repo" "$pr" "${sha:0:8}"
done

exit "$fail"
