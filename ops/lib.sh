#!/usr/bin/env bash
# jeryu-ctl/lib.sh — shared helpers for the host-side jeryu control plane.
#
# These scripts orchestrate the jeryu loop using only the THREE proven jeryu
# primitives (smart-HTTP git server, push->CI bridge, REST check-run/PR records)
# plus a real fast-forward git push for the actual main advance. They live on the
# host OUTSIDE the repos (so they do not add CI workflows that trip jankurai caps)
# and OUTSIDE the jeryu source tree (which Codex actively edits).
#
# Override via env:
#   JERYU_BASE      base URL of the jeryu-api server   (default canonical :8787)
#   JERYU_GIT_ROOT  on-disk <data-dir>/git for bare repos
set -uo pipefail

JERYU_BASE="${JERYU_BASE:-http://127.0.0.1:8787}"
JERYU_GIT_ROOT="${JERYU_GIT_ROOT:-/home/ubuntu/.local/share/jeryu/git}"
JERYU_CTL_STATE="${JERYU_CTL_STATE:-/home/ubuntu/.jeryu/agent-review}"

c_red=$'\033[31m'; c_grn=$'\033[32m'; c_ylw=$'\033[33m'; c_cyn=$'\033[36m'; c_off=$'\033[0m'
say()  { printf '%s[jeryu-ctl]%s %s\n' "$c_cyn" "$c_off" "$*" >&2; }
ok()   { printf '%s[jeryu-ctl]%s %s%s%s\n' "$c_cyn" "$c_off" "$c_grn" "$*" "$c_off" >&2; }
warn() { printf '%s[jeryu-ctl]%s %s%s%s\n' "$c_cyn" "$c_off" "$c_ylw" "$*" "$c_off" >&2; }
die()  { printf '%s[jeryu-ctl]%s %s%s%s\n' "$c_cyn" "$c_off" "$c_red" "$*" "$c_off" >&2; exit 1; }

# Bare repo path on disk for direct git reads (diff, merge-base, FF push target).
bare_path() { printf '%s/%s/%s.git' "$JERYU_GIT_ROOT" "$1" "$2"; }

j_health() { curl -fsS --max-time 5 "$JERYU_BASE/health" >/dev/null 2>&1; }

# All check-runs for a commit, as compact JSON array.
check_runs_json() {
  local owner="$1" repo="$2" sha="$3"
  python3 - "$JERYU_BASE" "$owner" "$repo" "$sha" <<'PY'
import json
import sys
import urllib.error
import urllib.request

base, owner, repo, sha = sys.argv[1:5]
items = []
page = 1
per_page = 100

while True:
    url = (
        f"{base}/repos/{owner}/{repo}/commits/{sha}/check-runs"
        f"?per_page={per_page}&page={page}"
    )
    try:
        with urllib.request.urlopen(url, timeout=10) as resp:
            data = json.load(resp)
    except Exception:
        break

    if isinstance(data, list):
        page_items = data
        total = None
    else:
        page_items = data.get("check_runs", data.get("items", []))
        total = data.get("total_count")

    if not page_items:
        break

    items.extend(page_items)
    if total is not None:
        try:
            if len(items) >= int(total):
                break
        except (TypeError, ValueError):
            pass
    if len(page_items) < per_page:
        break
    page += 1

print(json.dumps({"total_count": len(items), "check_runs": items}))
PY
}

# NOTE: the jeryu /commits/{sha}/check-runs endpoint returns ALL of the repo's
# check-runs (it does NOT filter by sha), and a (sha,name) pair can have several
# entries from re-runs. So every helper below filters by head_sha client-side and
# takes the LATEST entry per name (by completed_at/started_at).

# Conclusion of a single named check on a sha (empty if absent).
check_conclusion() {
  local owner="$1" repo="$2" sha="$3" name="$4"
  check_runs_json "$owner" "$repo" "$sha" | python3 -c '
import sys, json
sha, name = sys.argv[1], sys.argv[2]
try: d = json.load(sys.stdin)
except Exception: sys.exit(0)
runs = [r for r in (d if isinstance(d, list) else d.get("check_runs", [])) if r.get("head_sha") == sha]
runs.sort(key=lambda r: (r.get("completed_at") or r.get("started_at") or ""))
latest = {}
for r in runs: latest[r.get("name")] = r.get("conclusion", "")
print(latest.get(name, ""))
' "$sha" "$name"
}

# True iff (for THIS sha) at least one ci/* check exists and ALL ci/* latest=success.
ci_green() {
  local owner="$1" repo="$2" sha="$3"
  check_runs_json "$owner" "$repo" "$sha" | python3 -c '
import sys, json
sha = sys.argv[1]
try: d = json.load(sys.stdin)
except Exception: sys.exit(1)
runs = [r for r in (d if isinstance(d, list) else d.get("check_runs", [])) if r.get("head_sha") == sha]
runs.sort(key=lambda r: (r.get("completed_at") or r.get("started_at") or ""))
latest = {}
for r in runs: latest[r.get("name")] = r.get("conclusion")
ci = [(n, c) for n, c in latest.items() if str(n).startswith("ci/")]
ok = len(ci) > 0 and all(c == "success" for _, c in ci)
sys.exit(0 if ok else 1)
' "$sha"
}

# Post a completed check-run with a conclusion (success|failure|neutral).
post_check() {
  local owner="$1" repo="$2" sha="$3" name="$4" conclusion="$5"
  curl -fsS --max-time 10 -X POST \
    "$JERYU_BASE/repos/$owner/$repo/check-runs" \
    -H 'content-type: application/json' \
    -d "$(python3 -c 'import json,sys; print(json.dumps({"name":sys.argv[1],"head_sha":sys.argv[2],"status":"completed","conclusion":sys.argv[3]}))' "$name" "$sha" "$conclusion")" \
    >/dev/null 2>&1
}

# Resolve a GitHub token for the offsite relay (neverhuman identity).
# Prefer the LIVE gh-authenticated token; the .git-credentials-jeryu file holds a
# stale gho_ token that github now rejects.
github_token() {
  if [[ -n "${GH_RELAY_TOKEN:-}" ]]; then printf '%s' "$GH_RELAY_TOKEN"; return; fi
  local t; t="$(gh auth token 2>/dev/null)"; [[ -n "$t" ]] && { printf '%s' "$t"; return; }
  local f=/home/ubuntu/.git-credentials-jeryu tok
  if [[ -r "$f" ]]; then
    tok="$(grep -m1 'x-access-token:' "$f" 2>/dev/null | sed -E 's#https://x-access-token:([^@]+)@.*#\1#')"
    [[ -n "$tok" ]] && printf '%s' "$tok"
  fi
}

# Abort only if a url.*.insteadOf rewrite points at the DEAD gitea endpoint
# (127.0.0.1:2224), which would hijack a github push to a corpse. Benign fetch
# rewrites (github.com/neverhuman -> local forge or file:// bare mirrors, used by
# CI to resolve cross-repo deps) are NOT a push-hijack risk and are allowed; the
# earlier over-broad `neverhuman|gitea` match false-positived on those.
guard_no_insteadof() {
  if git config --get-regexp 'url\..*\.insteadof' 2>/dev/null | grep -qiE '127\.0\.0\.1:2224|/git/gitea/'; then
    die "ABORT: a url.*.insteadOf rewrite points at the dead gitea (127.0.0.1:2224) — it would hijack pushes. Remove it before relaying."
  fi
}
