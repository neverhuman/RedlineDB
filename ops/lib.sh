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
#   JERYU_GIT_ROOT  on-disk <data-dir>/git for bare repos
set -uo pipefail

JERYU_GIT_ROOT="${JERYU_GIT_ROOT:-/home/ubuntu/.local/share/jeryu/git}"
JERYU_CTL_STATE="${JERYU_CTL_STATE:-/home/ubuntu/.jeryu/agent-review}"

c_red=$'\033[31m'; c_grn=$'\033[32m'; c_ylw=$'\033[33m'; c_cyn=$'\033[36m'; c_off=$'\033[0m'
say()  { printf '%s[jeryu-ctl]%s %s\n' "$c_cyn" "$c_off" "$*" >&2; }
ok()   { printf '%s[jeryu-ctl]%s %s%s%s\n' "$c_cyn" "$c_off" "$c_grn" "$*" "$c_off" >&2; }
warn() { printf '%s[jeryu-ctl]%s %s%s%s\n' "$c_cyn" "$c_off" "$c_ylw" "$*" "$c_off" >&2; }
die()  { printf '%s[jeryu-ctl]%s %s%s%s\n' "$c_cyn" "$c_off" "$c_red" "$*" "$c_off" >&2; exit 1; }

# Bare repo path on disk for direct git reads (diff, merge-base, FF push target).
bare_path() { printf '%s/%s/%s.git' "$JERYU_GIT_ROOT" "$1" "$2"; }

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
