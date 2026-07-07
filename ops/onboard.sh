#!/usr/bin/env bash
# jeryu-ctl/onboard.sh — host a repo on jeryu so it can be the control plane.
# Creates the forge repo (materializes the bare repo on disk), adds a `jeryu` git
# remote to the working copy (origin=github stays as the safety net), and optionally
# seeds refs/heads/main from the working copy's local refs/heads/main. Idempotent.
#
# Usage: onboard.sh <repo_path> <jeryu_owner/name> [--push] [--seed-ref refs/heads/main]
#   e.g. onboard.sh /home/ubuntu/veox-split/veox-proofs jeryu/veox-proofs --push
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/lib.sh"
REPO_PATH="${1:?repo_path}"; FULL="${2:?owner/name}"; shift 2; PUSH=0; FLIP=0; SEED_REF="refs/heads/main"
while [[ $# -gt 0 ]]; do case "$1" in
  --push) PUSH=1 ;;
  --flip-origin) FLIP=1 ;;
  --seed-ref) shift; SEED_REF="${1:-}" ;;
  *) die "unknown arg: $1" ;;
esac; shift; done
OWNER="${FULL%%/*}"; NAME="${FULL#*/}"
[[ -d "$REPO_PATH/.git" ]] || die "not a git repo: $REPO_PATH"
[[ -n "$SEED_REF" ]] || die "--seed-ref requires a ref"
j_health || die "jeryu not healthy at $JERYU_BASE"
guard_no_insteadof

# 1. Create the forge repo (idempotent: 201 new, or already-exists is fine).
code="$(curl -s -o /dev/null -w '%{http_code}' -X POST "$JERYU_BASE/repos" \
  -H 'content-type: application/json' \
  -d "$(python3 -c 'import json,sys;print(json.dumps({"name":sys.argv[1],"private":True,"default_branch":"main"}))' "$NAME")")"
case "$code" in
  201) ok "created forge repo $OWNER/$NAME" ;;
  409|422) say "forge repo $OWNER/$NAME already exists" ;;
  *) warn "POST /repos returned HTTP $code (continuing; will verify bare repo)" ;;
esac
bare="$(bare_path "$OWNER" "$NAME")"
[[ -d "$bare" ]] || die "bare repo did not materialize at $bare (repo_create may be blocked) — fallback: git init --bare"

# 2. Remotes: ensure a `github` backup remote (captured from the current github origin),
#    a `jeryu` remote, and (with --flip-origin) point origin INTERNALLY at jeryu.
url="$JERYU_BASE/git/$OWNER/$NAME.git"
cur_origin="$(git -C "$REPO_PATH" remote get-url origin 2>/dev/null || echo '')"
if ! git -C "$REPO_PATH" remote get-url github >/dev/null 2>&1; then
  case "$cur_origin" in
    *github.com*) git -C "$REPO_PATH" remote add github "$cur_origin"; ok "remote 'github' -> $cur_origin (offsite backup / relay target)";;
    *) warn "current origin is not github ($cur_origin); set a 'github' remote manually for the relay";;
  esac
fi
git -C "$REPO_PATH" remote get-url jeryu >/dev/null 2>&1 \
  && git -C "$REPO_PATH" remote set-url jeryu "$url" \
  || git -C "$REPO_PATH" remote add jeryu "$url"
if [[ "$FLIP" == "1" ]]; then
  git -C "$REPO_PATH" remote set-url origin "$url"
  ok "origin -> $url  (POINTING INTERNALLY at jeryu; github backup preserved)"
else
  ok "remote 'jeryu' -> $url (origin unchanged)"
fi

# 3. Optionally seed the default branch. This deliberately reads the local main
#    ref, not HEAD, so dirty feature branches stay untouched during wave-2 onboarding.
if [[ "$PUSH" == "1" ]]; then
  seed_sha="$(git -C "$REPO_PATH" rev-parse --verify "$SEED_REF^{commit}" 2>/dev/null)" \
    || die "seed ref $SEED_REF not found in $REPO_PATH"
  cur_sha="$(git -C "$bare" rev-parse --verify refs/heads/main 2>/dev/null || true)"
  tmp_ref="refs/jeryu-seed/$NAME"
  say "seeding jeryu main from $SEED_REF @ ${seed_sha:0:12} ..."
  git -C "$bare" fetch -q --no-tags "$REPO_PATH" "$SEED_REF:$tmp_ref" \
    || die "failed to fetch $SEED_REF into $bare"
  if [[ -z "$cur_sha" ]]; then
    git -C "$bare" update-ref refs/heads/main "$seed_sha" \
      || die "failed to create refs/heads/main at ${seed_sha:0:12}"
    ok "seeded jeryu main from $SEED_REF @ ${seed_sha:0:12}"
  elif [[ "$cur_sha" == "$seed_sha" ]]; then
    ok "jeryu main already matches $SEED_REF @ ${seed_sha:0:12}"
  elif git -C "$bare" merge-base --is-ancestor "$cur_sha" "$seed_sha" 2>/dev/null; then
    git -C "$bare" update-ref refs/heads/main "$seed_sha" "$cur_sha" \
      || die "failed to fast-forward refs/heads/main to ${seed_sha:0:12}"
    ok "fast-forwarded jeryu main from ${cur_sha:0:12} to ${seed_sha:0:12}"
  else
    git -C "$bare" update-ref -d "$tmp_ref" >/dev/null 2>&1 || true
    die "jeryu main ${cur_sha:0:12} is not an ancestor of $SEED_REF ${seed_sha:0:12}; refusing to rewrite"
  fi
  git -C "$bare" update-ref -d "$tmp_ref" >/dev/null 2>&1 || true
fi
echo "$OWNER/$NAME"
