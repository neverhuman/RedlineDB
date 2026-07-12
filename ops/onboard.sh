#!/usr/bin/env bash
# jeryu-ctl/onboard.sh — host a repo on Jeryu with one managed origin.
# Creates the forge repo, points the working copy at the declared local-Jeryu
# remote, and optionally seeds a review branch. Idempotent.
#
# Usage: onboard.sh <repo_path> <owner/name> [--push] [--open-pr] [--seed-ref refs/heads/main]
#   e.g. onboard.sh /home/ubuntu/veox-split/veox-proofs jeryu/veox-proofs --push
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/lib.sh"
REPO_PATH="${1:?repo_path}"; FULL="${2:?owner/name}"; shift 2; PUSH=0; OPEN_PR=0; FLIP=0; SEED_REF="refs/heads/main"; SEED_BRANCH="onboard/v8.0.0"; MANIFEST="${JAIN_SPLIT_MANIFEST:-$HERE/../repos.manifest.toml}"; REQUIRED_CHECK="${JAIN_REQUIRED_CHECK:-}"
while [[ $# -gt 0 ]]; do case "$1" in
  --push) PUSH=1 ;;
  --open-pr) OPEN_PR=1 ;;
  --flip-origin) FLIP=1 ;;
  --seed-ref) shift; SEED_REF="${1:-}" ;;
  --seed-branch) shift; SEED_BRANCH="${1:-}" ;;
  --manifest) shift; MANIFEST="${1:-}" ;;
  --required-check) shift; REQUIRED_CHECK="${1:-}" ;;
  *) die "unknown arg: $1" ;;
esac; shift; done
OWNER="${FULL%%/*}"; NAME="${FULL#*/}"
[[ -d "$REPO_PATH/.git" ]] || die "not a git repo: $REPO_PATH"
[[ -n "$SEED_REF" ]] || die "--seed-ref requires a ref"
[[ "$SEED_BRANCH" != "main" ]] || die "direct main seeding is prohibited; push a review branch and merge through Jeryu"
j_health || die "jeryu not healthy at $JERYU_BASE"
guard_no_insteadof

# Required status checks are a repository declaration, not a SmartCluster
# special case. The canonical manifest covers Jain family and infrastructure
# repositories; nested-family callers pass --required-check explicitly.
if [[ -z "$REQUIRED_CHECK" ]]; then
  if [[ "$FULL" == "jeryu/jain-split-ops" || "$FULL" == "jain-split/jain-split-ops" ]]; then
    REQUIRED_CHECK="jain-split-ops/required"
  elif [[ -r "$MANIFEST" ]]; then
    splitctl_manifest="$HERE/../target/debug/splitctl"
    if [[ ! -x "$splitctl_manifest" ]]; then
      splitctl_manifest="$(command -v splitctl || true)"
    fi
    [[ -n "$splitctl_manifest" ]] || die "required check is not declared; pass --required-check"
    REQUIRED_CHECK="$("$splitctl_manifest" manifest --manifest "$MANIFEST" --json 2>/dev/null | jq -r --arg name "$NAME" '([.repo[]?, (.infrastructure_repo[]?)] | .[] | select(.name == $name) | .required_check) // empty')"
  fi
fi
[[ -n "$REQUIRED_CHECK" ]] || die "required check is not declared for $FULL; pass --required-check"

# Writes require the host-provided merge credential. Resolve it through the
# configured secret-store path only; never print or scrape the credential.
jeryu_token() {
  if [[ -n "${JERYU_MERGE_TOKEN:-}" ]]; then
    printf '%s' "$JERYU_MERGE_TOKEN"
    return
  fi
  local f="${JERYU_MERGE_TOKEN_FILE:-$HOME/.jeryu/secrets/merge-token}"
  [[ -r "$f" ]] && tr -d '\n' < "$f"
}

auth_args=()
token="$(jeryu_token)"
[[ -n "$token" ]] || die "local forge write credential unavailable; repair it through the configured Jeryu secret-store workflow"
auth_args=(-H "Authorization: Bearer $token")

# 1. Create the forge repo (idempotent: 201 new, or already-exists is fine).
code="$(curl -s -o /dev/null -w '%{http_code}' -X POST "$JERYU_BASE/repos" \
  "${auth_args[@]}" \
  -H 'content-type: application/json' \
  -d "$(jq -cn --arg name "$NAME" --arg owner "$OWNER" '{name:$name,owner:$owner,private:true,default_branch:"main"}')")"
case "$code" in
  201) ok "created forge repo $OWNER/$NAME" ;;
  409|422) say "forge repo $OWNER/$NAME already exists" ;;
  *) warn "POST /repos returned HTTP $code (continuing; will verify bare repo)" ;;
esac
# 2. Remotes: point origin at the declared local-Jeryu URL. Managed checkouts
#    intentionally have no unmanaged backup remotes.
url="$JERYU_BASE/git/$OWNER/$NAME.git"
git ls-remote "$url" >/dev/null 2>&1 \
  || die "forge repository $OWNER/$NAME exists in the API but its Git remote is not reachable"
if [[ "$FLIP" == "1" ]]; then
  if git -C "$REPO_PATH" remote get-url origin >/dev/null 2>&1; then
    git -C "$REPO_PATH" remote set-url origin "$url"
  else
    git -C "$REPO_PATH" remote add origin "$url"
  fi
  while IFS= read -r remote; do
    [[ "$remote" == "origin" || -z "$remote" ]] && continue
    git -C "$REPO_PATH" remote remove "$remote"
  done < <(git -C "$REPO_PATH" remote)
  ok "origin -> $url  (managed local-Jeryu origin only)"
else
  ok "declared local-Jeryu remote: $url (origin unchanged)"
fi

# 3. Optionally seed the default branch. This deliberately reads the local main
#    ref, not HEAD, so dirty feature branches stay untouched during wave-2 onboarding.
if [[ "$PUSH" == "1" ]]; then
  seed_sha="$(git -C "$REPO_PATH" rev-parse --verify "$SEED_REF^{commit}" 2>/dev/null)" \
    || die "seed ref $SEED_REF not found in $REPO_PATH"
  say "seeding review branch $SEED_BRANCH from $SEED_REF @ ${seed_sha:0:12} ..."
  git -C "$REPO_PATH" -c "http.extraHeader=Authorization: Bearer $token" push "$url" "$SEED_REF:refs/heads/$SEED_BRANCH" \
    || die "failed to seed forge review branch from $SEED_REF"
  ok "seeded review branch $SEED_BRANCH from $SEED_REF @ ${seed_sha:0:12}"
  if [[ "$OPEN_PR" == "1" ]]; then
    curl -fsS -X POST "$JERYU_BASE/repos/$OWNER/$NAME/pulls" \
      "${auth_args[@]}" -H 'content-type: application/json' \
      -d "$(jq -cn --arg title "onboard $NAME for Jain v8.0.0" --arg head "$SEED_BRANCH" --arg body "Materialized reviewed Jain v8.0.0 source snapshot." '{title:$title,head:$head,base:"main",body:$body,draft:true,actor:"codex"}')" >/dev/null \
      || die "failed to open onboarding pull request"
    ok "opened onboarding pull request $OWNER/$NAME $SEED_BRANCH -> main"
  fi
fi

# Protection is a forge capability, not a local Git convention. Fail closed if
# the API cannot configure and read back the immutable-main policy.
protection="$JERYU_BASE/repos/$OWNER/$NAME/branches/main/protection"
policy="$(jq -cn --arg check "$REQUIRED_CHECK" '{required_status_checks:[$check],required_approving_review_count:1,required_linear_history:true,enforce_admins:true,allow_force_pushes:false,allow_deletions:false}')"
curl -fsS -X PUT "$protection" "${auth_args[@]}" \
  -H 'content-type: application/json' -d "$policy" >/dev/null \
  || die "forge does not support or rejected branch protection for $OWNER/$NAME"
readback="$(curl -fsS "$protection" "${auth_args[@]}")" \
  || die "unable to read back branch protection for $OWNER/$NAME"
jq -e --arg check "$REQUIRED_CHECK" '
  ((if (.required_status_checks | type) == "array" then .required_status_checks else (.required_status_checks.contexts // []) end | index($check)) != null)
  and ((.required_approving_review_count // .required_pull_request_reviews.required_approving_review_count // 0) >= 1)
  and ((if (.required_linear_history | type) == "object" then .required_linear_history.enabled else .required_linear_history end) == true)
  and ((if (.enforce_admins | type) == "object" then .enforce_admins.enabled else .enforce_admins end) == true)
  and ((if (.allow_force_pushes | type) == "object" then .allow_force_pushes.enabled else .allow_force_pushes end) == false)
  and ((if (.allow_deletions | type) == "object" then .allow_deletions.enabled else .allow_deletions end) == false)
' <<<"$readback" >/dev/null \
  || die "branch protection readback did not satisfy the v8 Smartcluster policy"
ok "protected main for $OWNER/$NAME"
echo "$OWNER/$NAME"
