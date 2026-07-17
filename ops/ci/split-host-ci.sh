#!/usr/bin/env bash
# split-host-ci.sh — host-native required-check runner for the jain split family.
#
# Modeled on veox-split/jain-ctl/host-ci.sh (jain stays the control plane;
# the host is the runner) with one split-family addition: the sha is checked
# out into a standalone physical clone alongside physical sibling clones, so the
# workspaces' local `[patch]` path dependencies (../jain-core/...) resolve
# exactly as they do in the canonical checkout layout.
#
# Usage: split-host-ci.sh <owner> <repo> <sha> <repo_path> [check_name]
set -uo pipefail

# Status authority is never accepted from the caller. Even a rejected
# credential must not survive long enough to reach the reviewed worker.
if [[ -v JERYU_BASE || -v JERYU_MERGE_TOKEN || -v JERYU_MERGE_TOKEN_FILE ]]; then
  unset JERYU_BASE JERYU_MERGE_TOKEN JERYU_MERGE_TOKEN_FILE
  printf '[split-host-ci] caller-provided forge credentials are forbidden; use the root publisher\n' >&2
  exit 2
fi
unset JAIN_BASE

OWNER="${1:?owner}"; REPO="${2:?repo}"; SHA="${3:?sha}"; REPO_PATH="${4:?repo_path}"
CHECK="${5:-$REPO/required}"
RUNNER_PATH="$(realpath -e -- "${BASH_SOURCE[0]}")" || exit 2
REVIEWED_RUNNER_NAME=.split-host-ci-reviewed

# Public entry has no publication code. It delegates bootstrap preparation to
# the reviewed unprivileged parent helper, whose only sudo transition is the
# one-argument sandbox broker. The root sandbox invokes this file under the
# reserved reviewed basename for worker mode below.
if [[ "${RUNNER_PATH##*/}" != "$REVIEWED_RUNNER_NAME" ]]; then
  ENTRY_OPS_ROOT="$(cd "$(dirname "$RUNNER_PATH")/../.." && pwd)"
  exec "$ENTRY_OPS_ROOT/ops/ci/split-host-ci-parent.sh" "$@"
fi

# Reviewed worker mode accepts only the root-owned, read-only authority mount
# created by host-ci-sandbox. Caller-owned state and checkout paths are invalid.

REEXEC_STATE="${JAIN_HOST_CI_REEXEC_STATE:-}"
SPLIT_ROOT="${JAIN_SPLIT_ROOT:-/home/ubuntu/jain-split}"
[[ "$REEXEC_STATE" == /opt/jain-ci/authority/reexec-state.json \
  && -f "$REEXEC_STATE" && ! -L "$REEXEC_STATE" \
  && "$(stat -c '%a:%h' -- "$REEXEC_STATE")" == '444:1' \
  && "$RUNNER_PATH" == /opt/jain-ci/authority/.split-host-ci-reviewed \
  && ! -L "$RUNNER_PATH" \
  && "$(stat -c '%a:%h' -- "$RUNNER_PATH")" == '555:1' ]] || exit 2
jq -e '
  select(.schema_version == "jain.host-ci-reexec/v4")
  | select(.source_root == "/opt/jain-ci/authority/control-plane")
  | select(.exact_root == "/opt/jain-ci/authority/control-plane")
  | select(.result_path | type == "string" and startswith("/"))
  | select(.splitctl_path == "/opt/jain-ci/authority/splitctl")
  | select(.commit | test("^[0-9a-f]{40}$"))' "$REEXEC_STATE" >/dev/null \
  || exit 2
SOURCE_OPS_ROOT="$(realpath -e -- "$(jq -er '.source_root' "$REEXEC_STATE")")" \
  || exit 2
OPS_ROOT="$(realpath -e -- "$(jq -er '.exact_root' "$REEXEC_STATE")")" \
  || exit 2
CONTROL_PLANE_COMMIT="$(jq -er '.commit' "$REEXEC_STATE")" || exit 2
CHILD_RESULT_PATH="$(jq -er '.result_path' "$REEXEC_STATE")" || exit 2
SPLITCTL_BIN="$(realpath -e -- "$(jq -er '.splitctl_path' "$REEXEC_STATE")")" \
  || exit 2
[[ "$SOURCE_OPS_ROOT" == "$OPS_ROOT" \
  && "$OPS_ROOT" == /opt/jain-ci/authority/control-plane \
  && -d "$OPS_ROOT/.git" && ! -L "$OPS_ROOT" \
  && "$SPLITCTL_BIN" == /opt/jain-ci/authority/splitctl \
  && "$(stat -c '%a:%h' -- "$SPLITCTL_BIN")" == '555:1' \
  && "$CHILD_RESULT_PATH" \
    == "${JAIN_HOST_CI_WRITABLE_ROOT:?}/worker-evidence.json" \
  && "$JAIN_HOST_CI_WRITABLE_ROOT" \
    == "$SPLIT_ROOT"/target/host-ci-sandboxes/split-host-ci-bootstrap.??????/writable \
  && ! -e "$CHILD_RESULT_PATH" \
  && "$(git -c core.fsmonitor=false -c core.hooksPath=/dev/null \
    -C "$OPS_ROOT" rev-parse --verify 'HEAD^{commit}')" \
    == "$CONTROL_PLANE_COMMIT" \
  && "$(sha256sum -- "$RUNNER_PATH" | cut -d' ' -f1)" \
    == "$(sha256sum -- "$OPS_ROOT/ops/ci/split-host-ci.sh" | cut -d' ' -f1)" ]] \
  || exit 2
unset JAIN_HOST_CI_REEXEC_STATE

verify_exact_control_plane_integrity() {
  local verified
  verified="$(
    bash "$OPS_ROOT/ops/ci/host-ci-integrity.sh" \
      "$OPS_ROOT" "$CONTROL_PLANE_COMMIT"
  )" || return 1
  [[ "$verified" == "$CONTROL_PLANE_COMMIT" ]]
}
verify_exact_control_plane_integrity || {
  printf '[split-host-ci] immutable control-plane integrity check failed\n' >&2
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
[ -d "$SPLIT_ROOT/jain-core" ] || { printf '[split-host-ci] JAIN_SPLIT_ROOT=%s is not a split family root (no jain-core/)\n' "$SPLIT_ROOT" >&2; exit 2; }

say() { printf '[split-host-ci] %s\n' "$*" >&2; }
# The reviewed worker has no status credential and never performs a forge
# write. It can return evidence metadata only; the root sandbox derives policy,
# seals the result, tears down the cgroup, and invokes the one-shot publisher.
post_check() {
  local conclusion="${1:?conclusion is required}" result_tmp
  [[ "$conclusion" == success ]] || return 0
  result_tmp="$CHILD_RESULT_PATH.tmp.$$"
  jq -n --arg owner "$OWNER" --arg repo "$REPO" --arg head_sha "$SHA" \
    --arg check "$CHECK" --arg commit "$CONTROL_PLANE_COMMIT" \
    --arg evidence_dir "${JAIN_NATIVE_EVIDENCE_DIR:-}" \
    --arg evidence_sha "${JAIN_NATIVE_EVIDENCE_SHA256:-}" \
    '{schema_version:"jain.host-ci-worker-evidence/v4",
      owner:$owner,repository:$repo,head_sha:$head_sha,required_check:$check,
      control_plane_commit:$commit,
      native_evidence_dir:$evidence_dir,
      native_evidence_sha256:$evidence_sha}' >"$result_tmp" || return 1
  chmod 0600 "$result_tmp" || return 1
  mv -- "$result_tmp" "$CHILD_RESULT_PATH" || return 1
  say 'recorded worker evidence for root policy validation'
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
  "$SPLITCTL_BIN" managed-repos --manifest "$CANONICAL_MANIFEST" --json
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
authority_mode=remote
[[ "${JAIN_HOST_CI_NETWORK_ISOLATED:-0}" == 1 ]] && authority_mode=local
jain_verify_reviewed_control_plane_commit \
  "$OPS_ROOT" "$CONTROL_PLANE_COMMIT" "$control_plane_remote" \
  "$authority_mode" || {
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
    "$SPLITCTL_BIN" release-cargo-commands \
      --manifest "$CANONICAL_MANIFEST" --repo "$REPO"
  )" || native_setup_failure \
    "failed to derive canonical release Cargo policy for $REPO" 2
fi

git -C "$REPO_PATH" cat-file -e "$SHA^{commit}" 2>/dev/null || {
  post_check failure || true
  echo "sha $SHA not in $REPO_PATH" >&2
  exit 2
}

checkout_root="${JAIN_HOST_CI_WRITABLE_ROOT:?}/physical-checkouts"
mkdir -p "$checkout_root" || exit 2
checkout_root="$(realpath -e -- "$checkout_root")" || exit 2
tmp="$(mktemp -d "$checkout_root/split-host-ci.XXXXXX")" || exit 2
wt="$tmp/$REPO"
native_vendor="$tmp/native-vendor"
native_source_input="${JAIN_NATIVE_SOURCE_ROOT:-$SPLIT_ROOT/vendor}"
native_source_root="$tmp/native-source"
native_bundle="$tmp/native-materializer"
native_authority=""
native_materializer=""
cleanup() {
  if [ -n "$native_authority" ] && [ -f "$native_authority" ]; then
    jain_cleanup_native_source_worktrees \
      "$native_authority" "$native_source_input" "$native_source_root"
  fi
  if [[ "$(realpath -e -- "$tmp" 2>/dev/null || true)" == "$tmp" \
    && "$tmp" == "$checkout_root"/split-host-ci.?????? \
    && -z "$(find "$tmp" -xdev -type l -print -quit 2>/dev/null)" \
    && -z "$(find "$tmp" -xdev ! -type d ! -type f -print -quit 2>/dev/null)" ]]; then
    rm -rf -- "$tmp" >/dev/null 2>&1 || true
  else
    say "refusing unsafe physical-checkout cleanup: $tmp"
  fi
}
trap cleanup EXIT

validate_physical_checkout() {
  local checkout="${1:?checkout is required}" expected="${2:?commit is required}"
  local checkout_real git_dir common_dir
  checkout_real="$(realpath -e -- "$checkout")" || return 1
  [[ "$checkout_real" == "$checkout" && -d "$checkout/.git" && ! -L "$checkout/.git" \
    && ! -e "$checkout/.git/commondir" && ! -e "$checkout/.git/worktrees" \
    && ! -e "$checkout/.git/objects/info/alternates" \
    && -z "$(find "$checkout" -xdev -type l -print -quit)" \
    && -z "$(find "$checkout" -xdev ! -type d ! -type f -print -quit)" ]] || return 1
  git_dir="$(git -C "$checkout" rev-parse --absolute-git-dir)" || return 1
  common_dir="$(git -C "$checkout" rev-parse --path-format=absolute --git-common-dir)" \
    || return 1
  [[ "$git_dir" == "$checkout/.git" && "$common_dir" == "$checkout/.git" \
    && "$(git -C "$checkout" rev-parse --verify 'HEAD^{commit}')" == "$expected" \
    && -z "$(git -C "$checkout" status --porcelain=v1 --untracked-files=all)" ]] \
    || return 1
}

git clone --quiet --no-local --no-checkout "$REPO_PATH" "$wt" \
  || { post_check failure; echo "physical checkout clone failed" >&2; exit 1; }
git -C "$wt" checkout --quiet --detach "$SHA" \
  || { post_check failure; echo "physical checkout failed" >&2; exit 1; }
git -C "$wt" remote remove origin || exit 1
validate_physical_checkout "$wt" "$SHA" \
  || { post_check failure; echo "physical checkout is not isolated" >&2; exit 1; }

# Release Cargo policy may enable native learners even when the repository's
# merge lane does not. Extract the materializer from the exact reviewed
# control-plane commit, stage clean worktrees from authority-bound Git objects,
# and preserve a checksummed receipt outside this disposable checkout.
if [ "${JAIN_RELEASE_CI:-0}" = "1" ] && [ "${#native_learners[@]}" -gt 0 ]; then
  jain_extract_native_materializer "$OPS_ROOT" "$native_bundle" \
    "$control_plane_remote" "$CONTROL_PLANE_COMMIT" "$authority_mode" \
    || native_setup_failure \
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
  : "${JAIN_NATIVE_EVIDENCE_STAGING_ROOT:?root evidence staging is required}"
  jain_persist_native_evidence \
    "$native_vendor" "$tmp/native-vendor.log" \
    "$JAIN_NATIVE_EVIDENCE_STAGING_ROOT" \
    "$tmp" "$OWNER" "$REPO" "$SHA" "$CHECK" \
    "$JAIN_NATIVE_CONTROL_COMMIT" "$OPS_ROOT" \
    || native_setup_failure \
    "release CI native materialization evidence persistence failed" 1
  say "native materialization receipt: $JAIN_NATIVE_EVIDENCE_DIR/receipt.json ($JAIN_NATIVE_EVIDENCE_SHA256)"
  mkdir -p "$wt/target" || native_setup_failure \
    "release CI native target setup failed" 1
  mv -- "$native_vendor" "$wt/target/native-vendor" || native_setup_failure \
    "release CI native vendor placement failed" 1
  native_vendor="$wt/target/native-vendor"
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
    if [ -d "$SPLIT_ROOT/$sib/.git" ]; then
      sib_sha="$(git -C "$SPLIT_ROOT/$sib" rev-parse --verify 'HEAD^{commit}')" \
        || native_setup_failure "cannot resolve sibling $sib" 1
      git clone --quiet --no-local --no-checkout "$SPLIT_ROOT/$sib" "$tmp/$sib" \
        || native_setup_failure "cannot clone sibling $sib" 1
      git -C "$tmp/$sib" checkout --quiet --detach "$sib_sha" \
        || native_setup_failure "cannot checkout sibling $sib" 1
      git -C "$tmp/$sib" remote remove origin \
        || native_setup_failure "cannot isolate sibling $sib" 1
      validate_physical_checkout "$tmp/$sib" "$sib_sha" \
        || native_setup_failure "sibling $sib is not a physical isolated checkout" 1
    fi
  done
fi

# Weight-hungry lanes (feat-core foundation/hyperion tests, starforge golden
# parity) opt into the real safetensors via JAIN_NEEDS_ARTIFACTS; feat-core's
# starforge_integration resolves repo-relative artifacts/ from the worktree root.
if [ "${JAIN_NEEDS_ARTIFACTS:-0}" = "1" ] && [ -d "$SPLIT_ROOT/jain-starforge/artifacts" ]; then
  cp -aL -- "$SPLIT_ROOT/jain-starforge/artifacts" "$wt/artifacts" \
    || native_setup_failure "artifact staging failed" 1
  [[ -z "$(find "$wt/artifacts" -xdev -type l -print -quit)" \
    && -z "$(find "$wt/artifacts" -xdev ! -type d ! -type f -print -quit)" ]] \
    || native_setup_failure "artifact staging contains symlink or special nodes" 1
fi

# Release CI uses a fresh Cargo home and target. The root broker exposes only a
# read-only registry archive/index cache; splitctl stages the exact crates.io
# inputs named by Cargo.lock after verifying every archive checksum.
if [ "${JAIN_RELEASE_CI:-0}" = "1" ]; then
  [[ "${JAIN_HOST_CI_NETWORK_ISOLATED:-0}" == 1 \
    && -d /opt/jain-ci/cargo-registry \
    && ! -L /opt/jain-ci/cargo-registry ]] \
    || native_setup_failure "release Cargo cache is not inside the isolated worker" 1
  export CARGO_HOME="$tmp/cargo-home"
  export CARGO_TARGET_DIR="$tmp/cargo-target"
  mkdir -m 0700 "$CARGO_HOME" "$CARGO_TARGET_DIR" \
    || native_setup_failure "cannot create fresh release Cargo directories" 1
  mapfile -d '' -t cargo_lock_paths < <(
    git -C "$wt" ls-files -z -- Cargo.lock ':(glob)**/Cargo.lock' | LC_ALL=C sort -z
  )
  root_lock_tracked=false
  cargo_lock_args=()
  for cargo_lock_path in "${cargo_lock_paths[@]}"; do
    [[ "$cargo_lock_path" == Cargo.lock ]] && root_lock_tracked=true
    cargo_lock_args+=(--lock "$wt/$cargo_lock_path")
  done
  if [ -f "$wt/Cargo.toml" ] && [ "$root_lock_tracked" != true ]; then
    native_setup_failure "release Rust repository has no tracked root Cargo.lock" 1
  fi
  if [ "${#cargo_lock_paths[@]}" -gt 0 ]; then
    "$SPLITCTL_BIN" cargo-cache-stage \
      "${cargo_lock_args[@]}" \
      --source /opt/jain-ci/cargo-registry \
      --destination "$CARGO_HOME/registry" \
      --receipt "$CARGO_HOME/registry/stage-receipt.json" \
      --expected-source-uid 0 --expected-source-gid 0 \
      || native_setup_failure "locked Cargo registry cache staging failed" 1
  fi
  export CARGO_NET_OFFLINE=true
  export CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
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
  ci_gitconfig="${JAIN_HOST_CI_WRITABLE_ROOT:-$SPLIT_ROOT/target}/ci-gitconfig"
  mkdir -p "$(dirname "$ci_gitconfig")"
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
  boundary_probe="${JAIN_HOST_CI_WRITABLE_ROOT:-}/boundary-probe.txt"
  if [[ -n "${JAIN_HOST_CI_WRITABLE_ROOT:-}" \
    && -f "$boundary_probe" && ! -L "$boundary_probe" \
    && "$(stat -c '%u:%a:%h:%s' -- "$boundary_probe" 2>/dev/null)" \
      =~ ^$(id -u):600:1:[0-9]{1,4}$ ]]; then
    cat "$boundary_probe" >&2
  fi
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
