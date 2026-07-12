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
  curl -fsS --max-time 10 \
    "$JERYU_BASE/repos/$owner/$repo/commits/$sha/check-runs?per_page=100"
}

# NOTE: the jeryu /commits/{sha}/check-runs endpoint returns ALL of the repo's
# check-runs (it does NOT filter by sha), and a (sha,name) pair can have several
# entries from re-runs. So every helper below filters by head_sha client-side and
# takes the LATEST entry per name (by completed_at/started_at).

# Conclusion of a single named check on a sha (empty if absent).
check_conclusion() {
  local owner="$1" repo="$2" sha="$3" name="$4"
  check_runs_json "$owner" "$repo" "$sha" |
    jq -r --arg sha "$sha" --arg name "$name" '
      [.check_runs[]? | select(.head_sha == $sha and .name == $name)]
      | sort_by(.completed_at // .started_at // "") | last.conclusion // empty'
}

# True iff (for THIS sha) at least one ci/* check exists and ALL ci/* latest=success.
ci_green() {
  local owner="$1" repo="$2" sha="$3"
  check_runs_json "$owner" "$repo" "$sha" |
    jq -e --arg sha "$sha" '
      [.check_runs[]? | select(.head_sha == $sha)]
      | sort_by(.completed_at // .started_at // "")
      | group_by(.name) | map(last)
      | map(select(.name | startswith("ci/")))
      | (length > 0 and all(.conclusion == "success"))' >/dev/null
}

# Post a completed check-run with a conclusion (success|failure|neutral).
post_check() {
  local owner="$1" repo="$2" sha="$3" name="$4" conclusion="$5"
  curl -fsS --max-time 10 -X POST \
    "$JERYU_BASE/repos/$owner/$repo/check-runs" \
    -H 'content-type: application/json' \
    -d "$(jq -cn --arg name "$name" --arg sha "$sha" --arg conclusion "$conclusion" \
      '{name:$name,head_sha:$sha,status:"completed",conclusion:$conclusion}')" \
    >/dev/null 2>&1
}

# Resolve a GitHub token for the offsite relay (neverhuman identity): the explicit
# env override, else the live gh-authenticated token. Deliberately NO credential-file
# scraping — the old third arm grepped /home/ubuntu/.git-credentials-jeryu (a stale
# gho_ token GitHub already rejects), which is both dead weight and exactly the
# credential-hunting pattern the access policy forbids. If neither source yields a
# token, callers fail closed; re-auth with `gh auth login`.
github_token() {
  if [[ -n "${GH_RELAY_TOKEN:-}" ]]; then printf '%s' "$GH_RELAY_TOKEN"; return; fi
  gh auth token 2>/dev/null || true
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
