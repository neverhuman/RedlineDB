#!/usr/bin/env bash
# jeryu-poll.sh — the autonomous control loop on the jeryu control plane.
#
# For each ONBOARDED repo in repos.manifest.toml, drive every open PR into main through
# the full local Jeryu gate, idempotently (skips work already done), then auto-merge
# and optionally best-effort mirror the signed Jeryu main commit to GitHub main:
#     open PR -> host-ci -> agent-review -> FF-merge -> release -> GitHub mirror
# Safe to run on a 2-min cron/timer.
#
# usage: jeryu-poll.sh [--max-wave N] [--parallel N] [--mirror-github] [--deploy]
#   --max-wave N   only repos up to rollout_wave N (default 1 = the 9 veox-split)
#   --parallel N   repos to process concurrently (default 3); per-repo work is serialized
#   --mirror-github after local Jeryu gates are green, best-effort mirror to GitHub main
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/lib.sh"; . "$HERE/manifest.sh"
MAXWAVE=1; PARALLEL=3; MIRROR_GITHUB=0; DEPLOY=0
while [[ $# -gt 0 ]]; do case "$1" in
  --max-wave) shift; MAXWAVE="${1:-1}";;
  --parallel) shift; PARALLEL="${1:-3}";;
  --mirror-github) MIRROR_GITHUB=1;;
  --deploy) DEPLOY=1;;   # fire post-merge signed deployment after each merge
  *) die "unknown arg: $1";;
esac; shift; done
[[ "$PARALLEL" =~ ^[0-9]+$ && "$PARALLEL" -ge 1 ]] || die "--parallel must be a positive integer"
j_health || die "jeryu not healthy at $JERYU_BASE"

# Concurrency guard: never overlap poll cycles (a heavy host-ci can exceed the 2-min tick).
exec 9>"/home/ubuntu/.jeryu/jeryu-poll.lock"
flock -n 9 || { say "another poll cycle holds the lock — skipping this tick"; exit 0; }

process_repo(){
  local name="$1" path="$2" slug="$3" jslug="$4"
  [[ -n "$jslug" ]] || return 0
  local owner repo bare pulls head_ref sha am_args main_sha
  owner="${jslug%%/*}"; repo="${jslug#*/}"
  bare="$(bare_path "$owner" "$repo")"
  [[ -d "$bare" ]] || { warn "$name: no bare repo on jeryu ($bare) — skip"; return 0; }
  say "── $name ($jslug) ──"
  pulls="$(curl -fsS --max-time 10 "$JERYU_BASE/repos/$owner/$repo/pulls?state=open" 2>/dev/null)"
  mapfile -t HEADS < <(printf '%s' "$pulls" | python3 -c '
import sys,json
try: d=json.load(sys.stdin)
except Exception: sys.exit(0)
for p in (d if isinstance(d,list) else []):
    if p.get("state")!="open" or p.get("draft"): continue
    if (p.get("base") or {}).get("ref")!="main": continue
    h=(p.get("head") or {}).get("ref")
    if h: print(h)')
  for head_ref in "${HEADS[@]}"; do
    [[ -n "$head_ref" ]] || continue
    sha="$(git -C "$bare" rev-parse --verify "refs/heads/$head_ref" 2>/dev/null)" || continue
    if [[ "$(check_conclusion "$owner" "$repo" "$sha" jeryu/ci)" != "success" ]]; then
      say "  $head_ref: running host-ci ..."; "$HERE/host-ci.sh" "$owner" "$repo" "$sha" "$path" >/dev/null 2>&1 || true
    fi
    if [[ "$(check_conclusion "$owner" "$repo" "$sha" jeryu/agent-review)" != "success" ]]; then
      say "  $head_ref: running agent-review ..."; "$HERE/agent-review.sh" "$owner" "$repo" "$sha" main >/dev/null 2>&1 || true
    fi
  done
  am_args=("$owner" "$repo")
  [[ "$DEPLOY" == "1" ]] && am_args+=(--deploy)
  "$HERE/automerge-jeryu.sh" "${am_args[@]}" 2>&1 | sed "s/^/  /"
  if [[ "$MIRROR_GITHUB" == "1" && -n "$slug" ]]; then
    main_sha="$(git -C "$bare" rev-parse --verify refs/heads/main 2>/dev/null)" || main_sha=""
    if [[ -n "$main_sha" ]]; then
      "$HERE/mirror-to-github-main.sh" "$owner" "$repo" "$main_sha" "$slug" 2>&1 | sed "s/^/  /"
    else
      warn "$name: no local jeryu main sha; github mirror skipped"
    fi
  fi
}

pids=(); fail=0
while IFS='|' read -r name path slug jslug; do
  while (( $(jobs -pr | wc -l) >= PARALLEL )); do sleep 1; done
  process_repo "$name" "$path" "$slug" "$jslug" &
  pids+=("$!")
done < <(manifest_repos --onboarded true --max-wave "$MAXWAVE" --rows)

for pid in "${pids[@]}"; do
  wait "$pid" || fail=1
done
[[ "$fail" == "0" ]] || die "poll cycle had repo worker failures"
ok "poll cycle complete (max-wave=$MAXWAVE parallel=$PARALLEL mirror-github=$MIRROR_GITHUB)"
