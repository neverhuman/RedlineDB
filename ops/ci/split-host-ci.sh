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
RUNNER_PATH="$(realpath -e -- "${BASH_SOURCE[0]}")" || exit 2
REVIEWED_RUNNER_NAME=.split-host-ci-reviewed

# The checkout used to invoke this script is bootstrap-only. Capture its clean
# commit, create a detached worktree from that Git object, and invoke a runner
# extracted from the same commit. Public entry always takes this path; caller
# environment cannot select reviewed mode or cleanup paths.
if [[ "${RUNNER_PATH##*/}" != "$REVIEWED_RUNNER_NAME" ]]; then
  ENTRY_OPS_ROOT="$(cd "$(dirname "$RUNNER_PATH")/../.." && pwd)"
  bootstrap_commit="$(
    "$ENTRY_OPS_ROOT/ops/ci/host-ci-integrity.sh" "$ENTRY_OPS_ROOT"
  )" || {
    printf '[split-host-ci] exact control-plane integrity check failed\n' >&2
    exit 2
  }
  bootstrap_root="$(mktemp -d /tmp/split-host-ci-bootstrap.XXXXXX)" || exit 2
  bootstrap_exact="$bootstrap_root/control-plane"
  bootstrap_state="$bootstrap_root/reexec-state.json"
  reviewed_runner="$bootstrap_root/$REVIEWED_RUNNER_NAME"
  cleanup_failed_bootstrap() {
    local root_real=""
    git -C "$ENTRY_OPS_ROOT" worktree remove --force \
      "$bootstrap_exact" >/dev/null 2>&1 || true
    root_real="$(realpath -e -- "$bootstrap_root" 2>/dev/null || true)"
    case "$root_real" in
      /tmp/split-host-ci-bootstrap.??????)
        if [[ "$root_real" == "$bootstrap_root" \
          && ! -L "$bootstrap_root" \
          && "$(stat -c '%u:%a' -- "$bootstrap_root" 2>/dev/null)" \
            == "$(id -u):700" ]]; then
          rm -rf -- "$bootstrap_root"
        fi
        ;;
    esac
  }
  trap cleanup_failed_bootstrap EXIT
  git -C "$ENTRY_OPS_ROOT" worktree add --quiet --detach \
    "$bootstrap_exact" "$bootstrap_commit" || exit 2
  exact_commit="$(
    git -C "$ENTRY_OPS_ROOT" show \
      "$bootstrap_commit:ops/ci/host-ci-integrity.sh" \
      | bash -s -- "$bootstrap_exact" "$bootstrap_commit"
  )" || exit 2
  [[ "$exact_commit" == "$bootstrap_commit" ]] || exit 2
  git -C "$ENTRY_OPS_ROOT" show \
    "$bootstrap_commit:ops/ci/split-host-ci.sh" >"$reviewed_runner" || exit 2
  [[ "$(sha256sum -- "$reviewed_runner" | cut -d' ' -f1)" == \
    "$(git -C "$ENTRY_OPS_ROOT" show \
      "$bootstrap_commit:ops/ci/split-host-ci.sh" | sha256sum | cut -d' ' -f1)" ]] \
    || exit 2
  chmod 0500 "$reviewed_runner"
  seal="$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')"
  [[ "$seal" =~ ^[0-9a-f]{64}$ ]] || exit 2
  jq -n --arg seal "$seal" --arg source_root "$ENTRY_OPS_ROOT" \
    --arg exact_root "$bootstrap_exact" --arg commit "$bootstrap_commit" \
    '{schema_version:"jain.host-ci-reexec/v1",seal:$seal,
      source_root:$source_root,exact_root:$exact_root,commit:$commit}' \
    >"$bootstrap_state" || exit 2
  chmod 0600 "$bootstrap_state"
  JAIN_HOST_CI_REEXEC_STATE="$bootstrap_state" \
  JAIN_HOST_CI_REEXEC_SEAL="$seal" \
    bash "$reviewed_runner" "$@"
  runner_rc=$?
  exit "$runner_rc"
fi

# Reviewed mode accepts only a sealed state file inside the exact private
# mktemp directory created above. It never cleans that path; the still-running
# public parent owns cleanup using its local variables.
REEXEC_STATE="${JAIN_HOST_CI_REEXEC_STATE:-}"
REEXEC_SEAL="${JAIN_HOST_CI_REEXEC_SEAL:-}"
[[ "$REEXEC_STATE" = /* && "$REEXEC_SEAL" =~ ^[0-9a-f]{64}$ \
  && -f "$REEXEC_STATE" && ! -L "$REEXEC_STATE" ]] || exit 2
REEXEC_STATE="$(realpath -e -- "$REEXEC_STATE")" || exit 2
BOOTSTRAP_ROOT="$(dirname "$REEXEC_STATE")"
case "$BOOTSTRAP_ROOT" in
  /tmp/split-host-ci-bootstrap.??????) ;;
  *) exit 2 ;;
esac
[[ ! -L "$BOOTSTRAP_ROOT" \
  && "$(stat -c '%u:%a' -- "$BOOTSTRAP_ROOT" 2>/dev/null)" \
    == "$(id -u):700" \
  && "$(stat -c '%u:%a:%h' -- "$REEXEC_STATE" 2>/dev/null)" \
    == "$(id -u):600:1" \
  && "$RUNNER_PATH" == "$BOOTSTRAP_ROOT/$REVIEWED_RUNNER_NAME" \
  && ! -L "$RUNNER_PATH" \
  && "$(stat -c '%u:%a:%h' -- "$RUNNER_PATH" 2>/dev/null)" \
    == "$(id -u):500:1" ]] || exit 2
jq -e --arg seal "$REEXEC_SEAL" \
  'select(.schema_version == "jain.host-ci-reexec/v1" and .seal == $seal)
   | select(.source_root | type == "string")
   | select(.exact_root | type == "string")
   | select(.commit | test("^[0-9a-f]{40}$"))' \
  "$REEXEC_STATE" >/dev/null || exit 2
SOURCE_OPS_ROOT="$(jq -er '.source_root' "$REEXEC_STATE")" || exit 2
OPS_ROOT="$(jq -er '.exact_root' "$REEXEC_STATE")" || exit 2
CONTROL_PLANE_COMMIT="$(jq -er '.commit' "$REEXEC_STATE")" || exit 2
SOURCE_OPS_ROOT="$(realpath -e -- "$SOURCE_OPS_ROOT")" || exit 2
OPS_ROOT="$(realpath -e -- "$OPS_ROOT")" || exit 2
jain_git_common_dir() {
  local root="${1:?Git root is required}" common
  common="$(git -C "$root" rev-parse --git-common-dir 2>/dev/null)" || return 1
  case "$common" in
    /*) realpath -e -- "$common" ;;
    *) realpath -e -- "$root/$common" ;;
  esac
}
[[ "$OPS_ROOT" == "$BOOTSTRAP_ROOT/control-plane" \
  && "$SOURCE_OPS_ROOT" != "$OPS_ROOT" \
  && -f "$OPS_ROOT/.git" && ! -L "$OPS_ROOT/.git" \
  && "$(jain_git_common_dir "$OPS_ROOT")" \
    == "$(jain_git_common_dir "$SOURCE_OPS_ROOT")" \
  && "$(git -C "$OPS_ROOT" rev-parse --verify 'HEAD^{commit}')" \
    == "$CONTROL_PLANE_COMMIT" ]] || exit 2
[[ "$(sha256sum -- "$RUNNER_PATH" | cut -d' ' -f1)" == \
  "$(git -C "$OPS_ROOT" show \
    "$CONTROL_PLANE_COMMIT:ops/ci/split-host-ci.sh" | sha256sum | cut -d' ' -f1)" ]] \
  || exit 2
unset JAIN_HOST_CI_REEXEC_STATE JAIN_HOST_CI_REEXEC_SEAL

verify_exact_control_plane_integrity() {
  local verified
  verified="$(
    git -C "$OPS_ROOT" show \
      "$CONTROL_PLANE_COMMIT:ops/ci/host-ci-integrity.sh" \
      | bash -s -- "$OPS_ROOT" "$CONTROL_PLANE_COMMIT"
  )" || return 1
  [[ "$verified" == "$CONTROL_PLANE_COMMIT" ]]
}

verify_exact_control_plane_integrity || {
  printf '[split-host-ci] detached control-plane integrity check failed\n' >&2
  exit 2
}
CANONICAL_MANIFEST="$OPS_ROOT/repos.manifest.toml"
# shellcheck source=ops/ci/native-runtime.sh
source "$OPS_ROOT/ops/ci/native-runtime.sh"
# shellcheck source=ops/ci/pinned-advisory.sh
source "$OPS_ROOT/ops/ci/pinned-advisory.sh"
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
  local conclusion="$1" token description
  token="$(jeryu_token)"
  if [ -z "$token" ]; then say "no merge token; cannot post required status"; return 1; fi
  if [[ "$conclusion" == success ]]; then
    verify_exact_control_plane_integrity || {
      say "control-plane bytes changed before success publication"
      return 1
    }
  fi
  jain_verify_native_check_evidence "$conclusion" \
    "${NATIVE_EVIDENCE_REQUIRED:-0}" "${JAIN_NATIVE_EVIDENCE_DIR:-}" \
    "${JAIN_NATIVE_EVIDENCE_SHA256:-}" "$SHA" "$CHECK" \
    "$OPS_ROOT" "$CONTROL_PLANE_COMMIT" || {
    say "native evidence requirement failed before status publication"
    return 1
  }
  # A check-run is the human-facing run record.
  curl -fsS -X POST "$JAIN_BASE/repos/$OWNER/$REPO/check-runs" \
    -H "Authorization: Bearer $token" \
    -H 'content-type: application/json' \
    -d "$(jq -cn --arg name "$CHECK" --arg sha "$SHA" --arg conclusion "$conclusion" \
      '{name:$name,head_sha:$sha,status:"completed",conclusion:$conclusion}')" \
    >/dev/null || return 1
  say "posted check-run $CHECK=$conclusion on ${SHA:0:8}"
  # Branch protection gates on a COMMIT STATUS (required_status_checks.contexts), which is a
  # DIFFERENT object from a check-run — without it a protected merge fails MissingStatusCheck.
  # The status must be keyed on the FULL head sha the PR records (short shas do not match).
  local status_state="failure"
  [ "$conclusion" = "success" ] && status_state="success"
  description="$CHECK via split-host-ci"
  if [ -n "${JAIN_NATIVE_EVIDENCE_SHA256:-}" ]; then
    description="$CHECK native-receipt=$JAIN_NATIVE_EVIDENCE_SHA256"
  fi
  curl -fsS -X POST "$JAIN_BASE/repos/$OWNER/$REPO/statuses/$SHA" \
    -H "Authorization: Bearer $token" \
    -H 'content-type: application/json' \
    -d "$(jq -cn --arg state "$status_state" --arg context "$CHECK" \
      --arg description "$description" \
      '{state:$state,context:$context,description:$description}')" \
    >/dev/null || return 1
  say "posted status $CHECK=$status_state on ${SHA:0:8}"
}

native_setup_failure() {
  local message="${1:?native setup failure message is required}"
  local rc="${2:-1}"
  printf '%s\n' "$message" >&2
  post_check failure || say "native setup failure status publication also failed"
  exit "$rc"
}

run_release_cargo_commands() {
  local policy="$1" count index program label subcommand
  shift
  local -a args=()
  local -a native_learners=("$@")
  count="$(jq -er '.commands | length | select(. > 0)' <<<"$policy")" || return 1
  for ((index = 0; index < count; index++)); do
    program="$(jq -er --argjson index "$index" '.commands[$index].program' <<<"$policy")" || return 1
    label="$(jq -er --argjson index "$index" '.commands[$index].label' <<<"$policy")" || return 1
    [ "$program" = "cargo" ] || {
      say "unsupported release command program: $program"
      return 1
    }
    args=()
    mapfile -t args < <(jq -er --argjson index "$index" '.commands[$index].args[]' <<<"$policy")
    [ "${#args[@]}" -gt 0 ] || {
      say "release cargo command has no arguments: $label"
      return 1
    }
    subcommand="${args[0]}"
    if [ "${#native_learners[@]}" -gt 0 ] && [ "$subcommand" = test ]; then
      say "verifying native runtime before release test: $label"
      jain_verify_native_libraries "$JAIN_VENDOR_ROOT" "${native_learners[@]}" || return 1
      jain_verify_linked_binaries "$CARGO_TARGET_DIR/release" || return 1
    fi
    say "running release cargo command: $label"
    cargo "${args[@]}" || return 1
    if [ "${#native_learners[@]}" -gt 0 ] && [ "$subcommand" = build ]; then
      say "verifying native runtime after release build: $label"
      jain_verify_native_libraries "$JAIN_VENDOR_ROOT" "${native_learners[@]}" || return 1
      jain_verify_linked_binaries "$CARGO_TARGET_DIR/release" || return 1
    fi
  done
}

[ -e "$REPO_PATH/.git" ] || { echo "not a git repo: $REPO_PATH" >&2; exit 2; }
command -v jq >/dev/null 2>&1 || { echo "host CI requires jq" >&2; exit 2; }
[[ "$SHA" =~ ^[0-9a-f]{40}$ ]] || { echo "host CI requires a full 40-hex SHA" >&2; exit 2; }
curl -fsS "$JAIN_BASE/health" >/dev/null || { echo "forge not healthy" >&2; exit 2; }
[ -n "$(jeryu_token)" ] || { echo "forge status credential is unavailable" >&2; exit 2; }

# Governed worker count (load-aware; never default high).
if command -v jain-ci-governor >/dev/null 2>&1; then
  JOBS="$(jain-ci-governor 2>/dev/null || echo 8)"
else
  JOBS="${JAIN_CI_JOBS:-8}"
fi
export JAIN_CI_JOBS="$JOBS" CARGO_BUILD_JOBS="$JOBS" WORKERS="$JOBS"
say "governed jobs=$JOBS"

native_learners=()
mapfile -t native_learners < <(jain_native_learners_for_repo "$REPO")
managed_inventory="$(
  cargo run --locked --quiet --manifest-path "$OPS_ROOT/Cargo.toml" -- \
    managed-repos --manifest "$CANONICAL_MANIFEST" --json
)" || {
  post_check failure || true
  echo "failed to derive canonical managed repository inventory" >&2
  exit 2
}
protected_check="$(jain_authoritative_required_check "$managed_inventory" "$REPO")" || {
  post_check failure || true
  echo "repository is absent or ambiguous in canonical managed inventory: $REPO" >&2
  exit 2
}
control_plane_remote="$(jain_authoritative_control_plane_remote "$managed_inventory")" || {
  post_check failure || true
  echo "control-plane remote is absent or ambiguous in canonical managed inventory" >&2
  exit 2
}
jain_verify_reviewed_control_plane_commit \
  "$OPS_ROOT" "$CONTROL_PLANE_COMMIT" "$control_plane_remote" || {
  post_check failure || true
  echo "control-plane commit is not authoritative reviewed main" >&2
  exit 2
}
NATIVE_EVIDENCE_REQUIRED=0
if jain_native_check_requires_evidence "$REPO" "$CHECK" "$protected_check"; then
  NATIVE_EVIDENCE_REQUIRED=1
fi
if ! jain_validate_native_check_mode \
  "$REPO" "$CHECK" "$protected_check" "${JAIN_RELEASE_CI:-0}"; then
  post_check failure || true
  exit 2
fi

release_cargo_policy=""
if [ "${JAIN_RELEASE_CI:-0}" = "1" ]; then
  release_cargo_policy="$(
    cargo run --locked --quiet --manifest-path "$OPS_ROOT/Cargo.toml" -- \
      release-cargo-commands --manifest "$CANONICAL_MANIFEST" --repo "$REPO"
  )" || native_setup_failure \
    "failed to derive canonical release Cargo policy for $REPO" 2
fi

git -C "$REPO_PATH" cat-file -e "$SHA^{commit}" 2>/dev/null || {
  post_check failure || true
  echo "sha $SHA not in $REPO_PATH" >&2
  exit 2
}

tmp="$(mktemp -d /tmp/split-host-ci.XXXXXX)"
wt="$tmp/$REPO"
native_vendor="$tmp/native-vendor"
native_source_input="${JAIN_NATIVE_SOURCE_ROOT:-$SPLIT_ROOT/vendor}"
native_source_root="$tmp/native-source"
native_bundle="$tmp/native-materializer"
native_authority=""
native_materializer=""
cleanup() {
  git -C "$REPO_PATH" worktree remove -f "$wt" >/dev/null 2>&1 || true
  if [ -n "$native_authority" ] && [ -f "$native_authority" ]; then
    jain_cleanup_native_source_worktrees \
      "$native_authority" "$native_source_input" "$native_source_root"
  fi
  rm -rf "$tmp" >/dev/null 2>&1 || true
}
trap cleanup EXIT

git -C "$REPO_PATH" worktree add -f --detach "$wt" "$SHA" >/dev/null 2>&1 \
  || { post_check failure; echo "worktree checkout failed" >&2; exit 1; }

# Release Cargo policy may enable native learners even when the repository's
# merge lane does not. Extract the materializer from the exact reviewed
# control-plane commit, stage clean worktrees from authority-bound Git objects,
# and preserve a checksummed receipt outside this disposable checkout.
if [ "${JAIN_RELEASE_CI:-0}" = "1" ] && [ "${#native_learners[@]}" -gt 0 ]; then
  jain_extract_native_materializer "$OPS_ROOT" "$native_bundle" \
    "$control_plane_remote" "$CONTROL_PLANE_COMMIT" || native_setup_failure \
    "release CI exact native materializer extraction failed" 1
  native_authority="$JAIN_NATIVE_AUTHORITY"
  native_materializer="$JAIN_NATIVE_MATERIALIZER"
  jain_stage_native_source_worktrees \
    "$native_authority" "$native_source_input" "$native_source_root" \
    || native_setup_failure "release CI exact native source staging failed" 1
  unset JAIN_NATIVE_SOURCE_ROOT JAIN_VENDOR_ROOT
  "$native_materializer" --authority "$native_authority" \
    --source-root "$native_source_root" --vendor-root "$native_vendor" \
    >"$tmp/native-vendor.log" 2>&1 || {
    cat "$tmp/native-vendor.log" >&2
    native_setup_failure "release CI exact native materialization failed" 1
  }
  verify_exact_control_plane_integrity || native_setup_failure \
    "control-plane bytes changed before native evidence persistence" 1
  jain_persist_native_evidence \
    "$native_vendor" "$tmp/native-vendor.log" \
    "${JAIN_NATIVE_EVIDENCE_ROOT:-$SPLIT_ROOT/target/host-ci-evidence/native-materialization}" \
    "$tmp" "$OWNER" "$REPO" "$SHA" "$CHECK" \
    "$JAIN_NATIVE_CONTROL_COMMIT" "$OPS_ROOT" \
    || native_setup_failure \
    "release CI native materialization evidence persistence failed" 1
  say "native materialization receipt: $JAIN_NATIVE_EVIDENCE_DIR/receipt.json ($JAIN_NATIVE_EVIDENCE_SHA256)"
  mkdir -p "$wt/target" || native_setup_failure \
    "release CI native target setup failed" 1
  ln -s "$native_vendor" "$wt/target/native-vendor" || native_setup_failure \
    "release CI native vendor link setup failed" 1
  jain_prepare_native_runtime "$native_vendor" || {
    native_setup_failure "release CI native runtime path setup failed" 1
  }
fi

# Independence by default: NO sibling repos are linked, so a repo's required lane
# must resolve cross-repo deps from its committed vendor-crates/ (offline). Only
# link siblings when a fleet/integration lane explicitly asks — jain-deploy's
# committed [patch] points at ../<sibling>/crates/..., and JAIN_NEEDS_SIBLINGS lets
# an integration lane opt in. Everything else stays sibling-free.
if [ "$REPO" = "jain-deploy" ] || [ "${JAIN_NEEDS_SIBLINGS:-0}" = "1" ]; then
  for sib in \
    jain jain-docs jain-domain jain-math jain-contracts jain-catboost \
    jain-xgboost jain-lightgbm jain-jable jain-battle-gpu jain-starforge \
    jain-core jain-llm jain-agent jain-jnoccio jain-zyal jain-jailgun \
    jain-research jain-report jain-tui jain-cli jain-web jain-python \
    jain-model-zoo jain-ops jain-smartcluster jain-deploy; do
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

# Release CI deliberately uses clean Cargo/target caches. Merge CI may retain
# its governed per-repository cache for latency.
if [ "${JAIN_RELEASE_CI:-0}" = "1" ]; then
  export CARGO_HOME="$tmp/cargo-home"
  export CARGO_TARGET_DIR="$tmp/cargo-target"
  mkdir -p "$CARGO_HOME" "$CARGO_TARGET_DIR"
elif [ -z "${CARGO_TARGET_DIR:-}" ]; then
  export CARGO_TARGET_DIR="${JAIN_CI_CACHE:-$HOME/.cache/jain-ci}/${OWNER}__${REPO}/target"
  mkdir -p "$CARGO_TARGET_DIR"
fi
if [ "${JAIN_RELEASE_CI:-0}" != "1" ] && [ -z "${RUSTC_WRAPPER:-}" ] && command -v sccache >/dev/null 2>&1; then
  export RUSTC_WRAPPER="$(command -v sccache)"
fi
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"

# Release security lanes consume an isolated checkout of one pinned RustSec
# commit. The cargo-audit/cargo-deny shims force advisory no-fetch operation, so
# a concurrent or dirty user advisory DB is neither read nor reset.
if [ "${JAIN_RELEASE_CI:-0}" = "1" ]; then
  rustsec_source="${JAIN_RUSTSEC_ADVISORY_SOURCE:-$SPLIT_ROOT/target/advisory-db}"
  rustsec_db="$CARGO_HOME/advisory-db"
  rustsec_tools="$tmp/pinned-rustsec-tools"
  real_cargo_audit="$(command -v cargo-audit)" || {
    native_setup_failure "release CI requires cargo-audit" 2
  }
  real_cargo_deny="$(command -v cargo-deny)" || {
    native_setup_failure "release CI requires cargo-deny" 2
  }
  jain_materialize_pinned_advisory_db \
    "$rustsec_source" "$rustsec_db" "$JAIN_PINNED_RUSTSEC_COMMIT" || {
    native_setup_failure "release CI pinned RustSec database setup failed" 1
  }
  jain_install_pinned_rustsec_tools \
    "$rustsec_tools" "$rustsec_db" "$CARGO_HOME" "$OPS_ROOT" || {
    native_setup_failure "release CI pinned RustSec tool setup failed" 1
  }
  export JAIN_REAL_CARGO_AUDIT="$real_cargo_audit"
  export JAIN_REAL_CARGO_DENY="$real_cargo_deny"
  export JAIN_PINNED_ADVISORY_DB="$rustsec_db"
  export JAIN_PINNED_ADVISORY_COMMIT="$JAIN_PINNED_RUSTSEC_COMMIT"
  export PATH="$rustsec_tools:$PATH"
  say "RustSec advisory database: isolated commit $JAIN_PINNED_RUSTSEC_COMMIT"
fi

# Cross-repo dependency resolution WITHOUT network fetches and WITHOUT sibling
# checkouts. Local Jeryu is the canonical operational source of truth, but CI
# resolves the exact same local-Jeryu tag URLs through auth-free file:// bare
# mirrors. This rewrite is scoped to the temp GIT_CONFIG_GLOBAL under target/;
# it never mutates repo config or ~/.gitconfig. Legacy GitHub internal URLs are
# also rewritten here only so older lockfiles fail less noisily while the family
# is being migrated. Bare mirrors are a credential-free CI cache, not canonical
# source.
if [ "$REPO" != "jain-split-ops" ]; then
  ci_gitconfig="$SPLIT_ROOT/target/ci-gitconfig"
  mkdir -p "$SPLIT_ROOT/target"
  printf '[url "file://%s/target/bare-mirrors/"]\n\tinsteadOf = http://127.0.0.1:8787/git/jeryu/\n\tinsteadOf = http://127.0.0.1:8787/git/jain-split/\n\tinsteadOf = http://127.0.0.1:8787/git/redline/\n\tinsteadOf = https://github.com/neverhuman/\n[net]\n\tgit-fetch-with-cli = true\n' "$SPLIT_ROOT" > "$ci_gitconfig"
  say "cross-repo resolution: local bare mirrors (CI cache for local Jeryu tags)"
  export GIT_CONFIG_GLOBAL="$ci_gitconfig"
fi

# Hermetic test env. The `jain` CLI asserts fail-closed behavior when no API URL
# is configured. A
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
  if [ "${JAIN_RELEASE_CI:-0}" = "1" ]; then
    if [ -f "$wt/Cargo.toml" ]; then
      (cd "$wt" && cargo metadata --locked --format-version 1 >/dev/null && \
        run_release_cargo_commands "$release_cargo_policy" "${native_learners[@]}") >>"$log" 2>&1 || {
        tail -30 "$log" >&2
        post_check failure || true
        say "FAIL release Cargo policy $OWNER/$REPO @ ${SHA:0:8}"
        exit 1
      }
    fi
    for lane in security score contract-drift artifact-support; do
      if (cd "$wt" && bash scripts/ci-local.sh "$lane") >>"$log" 2>&1; then
        continue
      fi
      tail -30 "$log" >&2
      post_check failure || true
      say "FAIL release $lane lane $OWNER/$REPO @ ${SHA:0:8}"
      exit 1
    done
  fi
  if ! post_check success; then
    say "CI passed but required success publication/evidence failed"
    post_check failure || say "required failure status publication also failed"
    exit 1
  fi
  say "PASS $OWNER/$REPO @ ${SHA:0:8}"
  exit 0
else
  rc=$?
  tail -30 "$log" >&2
  post_check failure || say "required failure status publication also failed"
  say "FAIL ($rc) $OWNER/$REPO @ ${SHA:0:8}"
  exit 1
fi
