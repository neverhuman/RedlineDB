#!/usr/bin/env bash
# split-host-ci.sh — host-native required-check runner for the jain split family.
#
# Modeled on veox-split/jain-ctl/host-ci.sh (jain stays the control plane;
# the host is the runner) with one split-family addition: the sha is checked
# out into a temp dir ALONGSIDE SYMLINKS to the sibling split repos, so the
# workspaces' local `[patch]` path dependencies (../jain-core/...) resolve
# exactly as they do in the canonical checkout layout.
#
# Usage: split-host-ci.sh <owner> <repo> <sha> <repo_path> [check_name]
set -uo pipefail

OWNER="${1:?owner}"; REPO="${2:?repo}"; SHA="${3:?sha}"; REPO_PATH="${4:?repo_path}"
CHECK="${5:-$REPO/required}"
JAIN_BASE="${JAIN_BASE:-http://127.0.0.1:8787}"
# The split family root (where the sibling repos + target/bare-mirrors live) is an
# EXPLICIT parameter, not derived from this script's location: this control-plane
# now lives in its own repo (jain-split-ops/), a sibling of the family members, so
# a location-derived root would be wrong. Callers (rollout-pr-flow.sh) pass it;
# default to the conventional root and assert it is really a family root.
SPLIT_ROOT="${JAIN_SPLIT_ROOT:-/home/ubuntu/jain-split}"
[ -d "$SPLIT_ROOT/jain-core" ] || { printf '[split-host-ci] JAIN_SPLIT_ROOT=%s is not a split family root (no jain-core/)\n' "$SPLIT_ROOT" >&2; exit 2; }

say() { printf '[split-host-ci] %s\n' "$*" >&2; }
# Posting a check-run is a WRITE, so the forge requires the local merge token (Bearer).
# JERYU_MERGE_TOKEN overrides; otherwise read the canonical token file. Without it the POST
# 401s and the run's status never reaches the forge.
jeryu_token() {
  if [ -n "${JERYU_MERGE_TOKEN:-}" ]; then printf '%s' "$JERYU_MERGE_TOKEN"; return; fi
  local f="${JERYU_MERGE_TOKEN_FILE:-$HOME/.jeryu/secrets/merge-token}"
  [ -r "$f" ] && tr -d '\n' < "$f"
}
post_check() {
  local conclusion="$1" token
  token="$(jeryu_token)"
  if [ -z "$token" ]; then say "WARN: no merge token; cannot post status"; return; fi
  # A check-run is the human-facing run record.
  curl -fsS -X POST "$JAIN_BASE/repos/$OWNER/$REPO/check-runs" \
    -H "Authorization: Bearer $token" \
    -H 'content-type: application/json' \
    -d "{\"name\":\"$CHECK\",\"head_sha\":\"$SHA\",\"status\":\"completed\",\"conclusion\":\"$conclusion\"}" \
    >/dev/null && say "posted check-run $CHECK=$conclusion on ${SHA:0:8}" || say "WARN: failed to post check-run"
  # Branch protection gates on a COMMIT STATUS (required_status_checks.contexts), which is a
  # DIFFERENT object from a check-run — without it a protected merge fails MissingStatusCheck.
  # The status must be keyed on the FULL head sha the PR records (short shas do not match).
  local status_state="failure"
  [ "$conclusion" = "success" ] && status_state="success"
  curl -fsS -X POST "$JAIN_BASE/repos/$OWNER/$REPO/statuses/$SHA" \
    -H "Authorization: Bearer $token" \
    -H 'content-type: application/json' \
    -d "{\"state\":\"$status_state\",\"context\":\"$CHECK\",\"description\":\"$CHECK via split-host-ci\"}" \
    >/dev/null && say "posted status $CHECK=$status_state on ${SHA:0:8}" || say "WARN: failed to post status"
}

[ -e "$REPO_PATH/.git" ] || { echo "not a git repo: $REPO_PATH" >&2; exit 2; }
curl -fsS "$JAIN_BASE/health" >/dev/null || { echo "forge not healthy" >&2; exit 2; }

# Governed worker count (load-aware; never default high).
if command -v jain-ci-governor >/dev/null 2>&1; then
  JOBS="$(jain-ci-governor 2>/dev/null || echo 8)"
else
  JOBS="${JAIN_CI_JOBS:-8}"
fi
export JAIN_CI_JOBS="$JOBS" CARGO_BUILD_JOBS="$JOBS" WORKERS="$JOBS"
say "governed jobs=$JOBS"

git -C "$REPO_PATH" cat-file -e "$SHA^{commit}" 2>/dev/null || { echo "sha $SHA not in $REPO_PATH" >&2; exit 2; }

tmp="$(mktemp -d /tmp/split-host-ci.XXXXXX)"
wt="$tmp/$REPO"
cleanup() {
  git -C "$REPO_PATH" worktree remove -f "$wt" >/dev/null 2>&1 || true
  rm -rf "$tmp" >/dev/null 2>&1 || true
}
trap cleanup EXIT

git -C "$REPO_PATH" worktree add -f --detach "$wt" "$SHA" >/dev/null 2>&1 \
  || { post_check failure; echo "worktree checkout failed" >&2; exit 1; }

# Independence by default: NO sibling repos are linked, so a repo's required lane
# must resolve cross-repo deps from its committed vendor-crates/ (offline). Only
# link siblings when a fleet/integration lane explicitly asks — jain-deploy's
# committed [patch] points at ../<sibling>/crates/..., and JAIN_NEEDS_SIBLINGS lets
# an integration lane opt in. Everything else stays sibling-free.
if [ "$REPO" = "jain-deploy" ] || [ "${JAIN_NEEDS_SIBLINGS:-0}" = "1" ]; then
  for sib in \
    jain jain-docs jain-domain jain-math jain-contracts jain-catboost \
    jain-xgboost jain-lightgbm jain-battle-gpu jain-starforge jain-core \
    jain-report jain-tui jain-cli jain-web jain-python jain-model-zoo \
    jain-ops jain-deploy; do
    [ "$sib" = "$REPO" ] && continue
    [ -d "$SPLIT_ROOT/$sib" ] && ln -s "$SPLIT_ROOT/$sib" "$tmp/$sib"
  done
fi

# Weight-hungry lanes (feat-core foundation/hyperion tests, starforge golden
# parity) opt into the real safetensors via JAIN_NEEDS_ARTIFACTS; feat-core's
# starforge_integration resolves repo-relative artifacts/ from the worktree root.
if [ "${JAIN_NEEDS_ARTIFACTS:-0}" = "1" ] && [ -d "$SPLIT_ROOT/jain-starforge/artifacts" ]; then
  ln -s "$SPLIT_ROOT/jain-starforge/artifacts" "$wt/artifacts"
fi

# Shared compile caches: persistent per-repo target dir + sccache when present,
# so the fresh worktree does not cold-compile the world (host-ci.sh precedent).
if [ -z "${CARGO_TARGET_DIR:-}" ]; then
  export CARGO_TARGET_DIR="${JAIN_CI_CACHE:-$HOME/.cache/jain-ci}/${OWNER}__${REPO}/target"
  mkdir -p "$CARGO_TARGET_DIR"
fi
if [ -z "${RUSTC_WRAPPER:-}" ] && command -v sccache >/dev/null 2>&1; then
  export RUSTC_WRAPPER="$(command -v sccache)"
fi
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"

# Cross-repo dependency resolution WITHOUT github and WITHOUT sibling checkouts.
# The split's git-tag deps use canonical github.com/neverhuman URLs, but nothing
# is published there; each consumer Cargo.lock pins the IMMUTABLE seed-tag SHA of
# every sibling. We rewrite those URLs (CI-scoped, via GIT_CONFIG_GLOBAL under
# target/, never ~/.gitconfig) to the local file:// bare mirrors, which carry every
# repo's seed release tag at exactly the pinned SHA. The mirrors are the primary
# resolution source because they are auth-free, local, and deterministic (proven:
# cargo --locked resolves the whole closure incl. all learner crates from them,
# identical crate graph, zero github). The forge git-http (JERYU_BASE) requires
# auth for reads, so it is NOT used for CI resolution — it is for human clones and
# is where main+tags are hosted. Set JAIN_CI_USE_FORGE=1 only if the forge is
# anonymously git-readable. This replaced the committed vendor-crates/ (which
# polluted jankurai as duplicated product); canonical source ids are unchanged so
# cargo tree is identical. guard_no_insteadof only runs on onboard/push, not here.
ci_gitconfig="$SPLIT_ROOT/target/ci-gitconfig"
mkdir -p "$SPLIT_ROOT/target"
if [ "${JAIN_CI_USE_FORGE:-0}" = "1" ] && git ls-remote "$JAIN_BASE/git/jeryu/jain-core.git" HEAD >/dev/null 2>&1; then
  printf '[url "%s/git/jeryu/"]\n\tinsteadOf = https://github.com/neverhuman/\n[net]\n\tgit-fetch-with-cli = true\n' "$JAIN_BASE" > "$ci_gitconfig"
  say "cross-repo resolution: local forge $JAIN_BASE (anonymously readable)"
else
  printf '[url "file://%s/target/bare-mirrors/"]\n\tinsteadOf = https://github.com/neverhuman/\n[net]\n\tgit-fetch-with-cli = true\n' "$SPLIT_ROOT" > "$ci_gitconfig"
  say "cross-repo resolution: local bare mirrors (auth-free)"
fi
export GIT_CONFIG_GLOBAL="$ci_gitconfig"

# Hermetic test env. jain-cli's dispatch tests assert fail-closed behavior when
# no API URL is configured (dispatch.rs falls back to $JAIN_API_URL). A
# forge-operator shell exports JAIN_API_URL=http://127.0.0.1:8787, which leaks
# into the test process and routes those tests at the LIVE forge (issue #6 not
# #1, repo-create -> Conflict, `status` exit 0 not 5). Clean GitHub CI never sets
# it; scrub it here so the host runner matches the GitHub runner.
unset JAIN_API_URL

if [[ -d "$SPLIT_ROOT/jain-starforge/.git/lfs" ]]; then
  export GIT_LFS_SKIP_SMUDGE=0
fi

log="$tmp/ci.log"
say "running scripts/ci-local.sh required for $OWNER/$REPO @ ${SHA:0:8}"
if (cd "$wt" && bash scripts/ci-local.sh required) >"$log" 2>&1; then
  post_check success
  say "PASS $OWNER/$REPO @ ${SHA:0:8}"
  exit 0
else
  rc=$?
  tail -30 "$log" >&2
  post_check failure
  say "FAIL ($rc) $OWNER/$REPO @ ${SHA:0:8}"
  exit 1
fi
