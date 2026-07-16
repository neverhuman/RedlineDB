#!/usr/bin/env bash
# split-host-ci.sh — host-native required-check runner for the jain split family.
#
# The runner always executes an unregistered, exact-SHA checkout. Integration
# lanes get independent exact-tag sibling clones from the CI bare-mirror cache;
# no canonical checkout is used as a build directory and no sibling path is
# linked into the sandbox.
#
# Usage: split-host-ci.sh <owner> <repo> <sha> <repo_path> [check_name] [--apply]
set -uo pipefail

JAIN_BASE="${JAIN_BASE:-http://127.0.0.1:8787}"
OPS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/host-ci-security.sh
source "$OPS_ROOT/ops/ci/host-ci-security.sh"
OWNER="${1:?owner}"
REPO="${2:?repo}"
SHA="${3:?sha}"
REPO_PATH="${4:?repo_path}"
shift 4
if [ "${1-}" = "--" ]; then
  shift
  CHECK="$REPO/required"
elif [ -n "${1-}" ] && [ "${1:0:2}" != "--" ]; then
  CHECK="$1"
  shift
else
  CHECK="$REPO/required"
fi
APPLY=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --apply)
      APPLY=1
      ;;
    --)
      ;;
    --*)
      printf '[split-host-ci] unknown option: %s\n' "$1" >&2
      exit 2
      ;;
    *)
      printf '[split-host-ci] unknown positional argument: %s\n' "$1" >&2
      exit 2
      ;;
  esac
  shift
done
CANONICAL_MANIFEST="$OPS_ROOT/repos.manifest.toml"
SPLIT_ROOT="${JAIN_SPLIT_ROOT:-/home/ubuntu/jain-split}"
MIRROR_ROOT="${JAIN_BARE_MIRROR_ROOT:-$SPLIT_ROOT/target/bare-mirrors}"
EVIDENCE_ROOT="${JAIN_CI_EVIDENCE_ROOT:-$OPS_ROOT/docs/release-evidence/8.0.1/ci/host}"

[ -d "$SPLIT_ROOT/jain-core" ] || {
  printf '[split-host-ci] JAIN_SPLIT_ROOT=%s is not a split family root (no jain-core/)\n' \
    "$SPLIT_ROOT" >&2
  exit 2
}
[ -f "$CANONICAL_MANIFEST" ] || {
  printf '[split-host-ci] canonical manifest is missing: %s\n' "$CANONICAL_MANIFEST" >&2
  exit 2
}

say() { printf '[split-host-ci] %s\n' "$*" >&2; }
STATUS_TOKEN=""

post_check() {
  local conclusion="$1" status_state
  jeryu_curl -fsS -X POST "$JAIN_BASE/repos/$OWNER/$REPO/check-runs" \
    -H 'content-type: application/json' \
    -d "{\"name\":\"$CHECK\",\"head_sha\":\"$SHA\",\"status\":\"completed\",\"conclusion\":\"$conclusion\"}" \
    >/dev/null || return 1
  say "posted check-run $CHECK=$conclusion on ${SHA:0:8}"

  status_state="failure"
  [ "$conclusion" = "success" ] && status_state="success"
  jeryu_curl -fsS -X POST "$JAIN_BASE/repos/$OWNER/$REPO/statuses/$SHA" \
    -H 'content-type: application/json' \
    -d "{\"state\":\"$status_state\",\"context\":\"$CHECK\",\"description\":\"$CHECK via split-host-ci\"}" \
    >/dev/null || return 1
  say "posted status $CHECK=$status_state on ${SHA:0:8}"
}

post_check_if_apply() {
  local conclusion="$1"
  if [ "$APPLY" -eq 1 ]; then
    post_check "$conclusion"
  else
    say "dry-run mode: skipping check publication ($conclusion) for $CHECK @ ${SHA:0:8}"
    return 0
  fi
}

manifest_value() {
  local field="$1" target="$2"
  awk -v field="$field" -v target="$target" '
    $0 == "[[repo]]" || $0 == "[[infrastructure_repo]]" { managed=1; name=""; next }
    $0 ~ /^\[\[/ { managed=0; name=""; next }
    managed && $1 == "name" && $2 == "=" {
      value=$0; sub(/^[^=]*= /, "", value); gsub(/^"|"$/, "", value); name=value; next
    }
    managed && name == target && $1 == field && $2 == "=" {
      value=$0; sub(/^[^=]*= /, "", value); gsub(/^"|"$/, "", value); print value; exit
    }
  ' "$CANONICAL_MANIFEST"
}

manifest_tag() {
  local target="$1" tag
  tag="$(manifest_value current_tag "$target")"
  [ -n "$tag" ] || tag="$(manifest_value immutable_tag "$target")"
  printf '%s' "$tag"
}

manifest_repos() {
  awk '
    $0 == "[[repo]]" || $0 == "[[infrastructure_repo]]" { managed=1; next }
    $0 ~ /^\[\[/ { managed=0; next }
    managed && $1 == "name" && $2 == "=" {
      value=$0; sub(/^[^=]*= /, "", value); gsub(/^"|"$/, "", value); print value
    }
  ' "$CANONICAL_MANIFEST"
}

mirror_for() {
  local name="$1" mirror
  mirror="$MIRROR_ROOT/$name.git"
  [ -d "$mirror" ] || { say "bare mirror is missing for $name: $mirror"; return 1; }
  [ ! -L "$mirror" ] || { say "bare mirror path is a symlink: $mirror"; return 1; }
  git --git-dir "$mirror" rev-parse --is-bare-repository 2>/dev/null | grep -qx true \
    || { say "bare mirror is not a bare repository: $mirror"; return 1; }
  printf '%s' "$mirror"
}

release_checksum() {
  local repo="$1" commit="$2"
  git -C "$repo" archive --format=tar "$commit" | sha256sum | awk '{print $1}'
}

verify_lock_identity() {
  local repo="$1" commit="$2" expected_blob actual_blob
  if git -C "$repo" cat-file -e "$commit:Cargo.lock" 2>/dev/null; then
    [ -f "$repo/Cargo.lock" ] || { say "Cargo.lock is absent from $repo"; return 1; }
    expected_blob="$(git -C "$repo" rev-parse "$commit:Cargo.lock")" || return 1
    actual_blob="$(git -C "$repo" hash-object "$repo/Cargo.lock")" || return 1
    [ "$expected_blob" = "$actual_blob" ] || {
      say "Cargo.lock blob mismatch in $repo: expected $expected_blob, got $actual_blob"
      return 1
    }
    printf 'lock_blob=%s\n' "$actual_blob" >> "$IDENTITY_LOG"
  else
    [ ! -e "$repo/Cargo.lock" ] || { say "unexpected Cargo.lock in $repo"; return 1; }
    printf 'lock_blob=absent\n' >> "$IDENTITY_LOG"
  fi
}

clone_exact() {
  local name="$1" source="$2" ref="$3" expected_commit="$4" dest="$5"
  local commit tree checksum expected_manifest_commit expected_checksum source_tree

  [ ! -e "$dest" ] || { say "clone destination already exists: $dest"; return 1; }
  # The bare mirror is an operator-owned CI cache; the runner uid need not match
  # its owner. Trust exactly this mirror path (never a glob) in the run-scoped
  # gitconfig so the read-only exact-SHA clone clears Git's dubious-ownership
  # guard without widening trust beyond the source actually being cloned.
  git config --file "$GIT_CONFIG_GLOBAL" --add safe.directory "$source"
  git clone --no-local --no-checkout "$source" "$dest" >> "$IDENTITY_LOG" 2>&1 || return 1
  git -C "$dest" checkout --detach --force "$ref" >> "$IDENTITY_LOG" 2>&1 || return 1
  commit="$(git -C "$dest" rev-parse --verify "HEAD^{commit}")" || return 1
  [ "$commit" = "$expected_commit" ] || {
    say "$name resolved $ref to $commit, expected $expected_commit"
    return 1
  }
  tree="$(git -C "$dest" rev-parse --verify "HEAD^{tree}")" || return 1
  source_tree="$(git --git-dir "$source" rev-parse --verify "$commit^{tree}")" || return 1
  [ "$tree" = "$source_tree" ] || { say "$name tree identity differs from its bare mirror"; return 1; }
  checksum="$(release_checksum "$dest" "$commit")" || return 1
  expected_manifest_commit="$(manifest_value release_commit "$name")"
  expected_checksum="$(manifest_value release_checksum_sha256 "$name")"
  if [ -n "$expected_manifest_commit" ] && [ "$expected_manifest_commit" != "PENDING" ]; then
    [ "$expected_manifest_commit" = "$commit" ] || {
      say "$name manifest commit $expected_manifest_commit differs from $commit"
      return 1
    }
  fi
  if [ -n "$expected_checksum" ] && [ "$expected_checksum" != "PENDING" ]; then
    [ "$expected_checksum" = "$checksum" ] || {
      say "$name manifest checksum $expected_checksum differs from $checksum"
      return 1
    }
  fi
  verify_lock_identity "$dest" "$commit" || return 1
  git -C "$dest" diff --quiet --exit-code || { say "$name clone is dirty after checkout"; return 1; }
  {
    printf 'name=%s\nref=%s\ncommit=%s\ntree=%s\nrelease_checksum_sha256=%s\n' \
      "$name" "$ref" "$commit" "$tree" "$checksum"
  } >> "$IDENTITY_LOG"
}

tmp=""
IDENTITY_LOG=""
log=""
# shellcheck disable=SC2317 # cleanup is called indirectly by EXIT trap.
cleanup() {
  [ -n "$tmp" ] || return 0
  if [ -n "$(find "$tmp" -type l -print -quit 2>/dev/null)" ]; then
    say "preserving sandbox with a symlink for manual review: $tmp"
    return 0
  fi
  rm -rf -- "$tmp"
}
trap cleanup EXIT

persist_log() {
  local conclusion="$1" log_sha receipt_dir receipt
  [ -f "$log" ] || return 0
  mkdir -p "$EVIDENCE_ROOT"
  log_sha="$(sha256sum "$log" | awk '{print $1}')"
  if [ ! -e "$EVIDENCE_ROOT/$log_sha.log" ]; then
    cp -- "$log" "$EVIDENCE_ROOT/$log_sha.log"
  fi
  receipt_dir="$EVIDENCE_ROOT"
  receipt="$receipt_dir/${REPO}-${SHA}-${log_sha}.json"
  if [ ! -e "$receipt" ]; then
    jq -n --arg schema 'jain.host-ci/v1' --arg owner "$OWNER" --arg repo "$REPO" \
      --arg sha "$SHA" --arg check "$CHECK" --arg conclusion "$conclusion" \
      --arg log_sha "$log_sha" --arg log_path "$EVIDENCE_ROOT/$log_sha.log" \
      '{schema_version:$schema,owner:$owner,repository:$repo,head_sha:$sha,check:$check,conclusion:$conclusion,log_sha256:$log_sha,log_path:$log_path}' \
      > "$receipt"
  fi
  say "content-addressed host-CI log $log_sha"
}

[ -e "$REPO_PATH/.git" ] || { say "not a git repo: $REPO_PATH"; exit 2; }
[[ "$SHA" =~ ^[0-9a-fA-F]{40}$ ]] || { say "head SHA must be a full 40-character commit: $SHA"; exit 2; }
curl -fsS "$JAIN_BASE/health" >/dev/null || { say "forge not healthy"; exit 2; }
if [ "$APPLY" -eq 1 ]; then
  unset STATUS_TOKEN
  STATUS_TOKEN="$(jeryu_token)"
  [ -n "$STATUS_TOKEN" ] || { say "forge status credential is unavailable"; exit 2; }
  unset JERYU_MERGE_TOKEN
  export -n STATUS_TOKEN 2>/dev/null || true
fi

if command -v jain-ci-governor >/dev/null 2>&1; then
  JOBS="$(jain-ci-governor 2>/dev/null || echo 1)"
else
  JOBS="${JAIN_CI_JOBS:-1}"
fi
export JAIN_CI_JOBS="$JOBS" CARGO_BUILD_JOBS="$JOBS" WORKERS="$JOBS"
say "governed jobs=$JOBS"

if [ "${JAIN_RELEASE_CI:-0}" = "1" ]; then
  cargo run --locked --quiet --manifest-path "$OPS_ROOT/Cargo.toml" -- \
    release-cargo-commands --manifest "$CANONICAL_MANIFEST" --repo "$REPO" \
    >/dev/null || { say "failed to derive canonical release Cargo policy for $REPO"; exit 2; }
fi

source_mirror="$(mirror_for "$REPO")" || exit 2
source_commit="$(git --git-dir "$source_mirror" rev-parse --verify "$SHA^{commit}" 2>/dev/null)" || {
  say "head $SHA is absent from the CI bare mirror $source_mirror; refresh the mirror before retrying"
  exit 2
}
[ "$source_commit" = "${SHA,,}" ] || { say "bare mirror resolved a different head: $source_commit"; exit 2; }

tmp="$(mktemp -d "${TMPDIR:-/tmp}/split-host-ci.XXXXXX")"
IDENTITY_LOG="$tmp/identity.log"
log="$tmp/ci.log"
mkdir -p "$tmp/home" "$tmp/cargo-home" "$tmp/cargo-target"
advisory_source="${JAIN_ADVISORY_DB:-$HOME/.cargo/advisory-db}"
touch "$IDENTITY_LOG"

# Use only sandbox-owned Git and Cargo state. The URL rewrites are limited to
# this run and keep local-Jeryu dependencies offline while preserving the
# canonical repository URLs in the cloned lockfiles.
gitconfig="$tmp/gitconfig"
git config --file "$gitconfig" --add url."file://$MIRROR_ROOT/".insteadOf http://127.0.0.1:8787/git/jeryu/
git config --file "$gitconfig" --add url."file://$MIRROR_ROOT/".insteadOf http://127.0.0.1:8787/git/jain-split/
git config --file "$gitconfig" --add url."file://$MIRROR_ROOT/".insteadOf http://127.0.0.1:8787/git/redline/
git config --file "$gitconfig" --add url."file://$MIRROR_ROOT/".insteadOf https://github.com/neverhuman/
git config --file "$gitconfig" net.git-fetch-with-cli true
export HOME="$tmp/home" CARGO_HOME="$tmp/cargo-home" CARGO_TARGET_DIR="$tmp/cargo-target"
export GIT_CONFIG_GLOBAL="$gitconfig" GIT_CONFIG_NOSYSTEM=1
export PATH="$CARGO_HOME/bin:$PATH"
unset JAIN_API_URL
if [ "${JAIN_RELEASE_CI:-0}" = "1" ]; then
  advisory_commit="$(tr -d '\r\n' < "$OPS_ROOT/ops/ci/rustsec-advisory-db.commit")"
  sandbox_advisory_db="$HOME/.cargo/advisory-db"
  seed_pinned_advisory_db "$advisory_source" "$sandbox_advisory_db" "$advisory_commit" || {
    say "failed to seed the clean pinned RustSec advisory database"
    exit 2
  }
  export JAIN_ADVISORY_DB="$sandbox_advisory_db"
  printf 'advisory_database_commit=%s\n' "$advisory_commit" >> "$IDENTITY_LOG"
fi

wt="$tmp/$REPO"
clone_exact "$REPO" "$source_mirror" "$SHA" "$source_commit" "$wt" || {
  say "exact target checkout failed"
  exit 1
}

native_source_root="${JAIN_NATIVE_SOURCE_ROOT:-}"
if [ "${JAIN_RELEASE_CI:-0}" = "1" ]; then
  native_bootstrap="$SPLIT_ROOT/jain-deploy/scripts/vendor-all.sh"
  [ -x "$native_bootstrap" ] || { say "native-vendor bootstrap missing: $native_bootstrap"; exit 2; }
  native_vendor="$tmp/native-vendor"
  unset JAIN_NATIVE_SOURCE_ROOT JAIN_VENDOR_ROOT
  bootstrap_args=(env "JAIN_VENDOR_ROOT=$native_vendor")
  [ -z "$native_source_root" ] || bootstrap_args+=("JAIN_NATIVE_SOURCE_ROOT=$native_source_root")
  "${bootstrap_args[@]}" bash "$native_bootstrap" > "$tmp/native-vendor.log" 2>&1 || {
    cat "$tmp/native-vendor.log" >&2
    say "release CI native-vendor bootstrap failed"
    exit 1
  }
  mkdir -p "$wt/target"
  cp -a "$native_vendor" "$wt/target/native-vendor"
fi

# Deploy and explicit integration lanes need sibling path dependencies. Every
# sibling is a separate clone at the manifest's immutable tag; a missing tag or
# mirror is a hard failure rather than a fallback to a canonical checkout.
if [ "$REPO" = "jain-deploy" ] || [ "${JAIN_NEEDS_SIBLINGS:-0}" = "1" ]; then
  while IFS= read -r sib; do
    [ -n "$sib" ] || continue
    [ "$sib" = "$REPO" ] && continue
    tag="$(manifest_tag "$sib")"
    [ -n "$tag" ] || { say "manifest has no current tag for sibling $sib"; exit 2; }
    sib_mirror="$(mirror_for "$sib")" || exit 2
    sib_commit="$(git --git-dir "$sib_mirror" rev-parse --verify "$tag^{commit}" 2>/dev/null)" || {
      say "manifest tag $tag is absent from the CI bare mirror for $sib"
      exit 2
    }
    clone_exact "$sib" "$sib_mirror" "$tag" "$sib_commit" "$tmp/$sib" || {
      say "exact sibling checkout failed for $sib@$tag"
      exit 1
    }
  done < <(manifest_repos)
fi

if [ "${JAIN_NEEDS_ARTIFACTS:-0}" = "1" ]; then
  [ -d "$SPLIT_ROOT/jain-starforge/artifacts" ] || { say "requested starforge artifacts are missing"; exit 2; }
  cp -a "$SPLIT_ROOT/jain-starforge/artifacts" "$wt/artifacts"
fi

cat "$IDENTITY_LOG" > "$log"
say "running scripts/ci-local.sh required for $OWNER/$REPO @ ${SHA:0:8}"
if (cd "$wt" && bash scripts/ci-local.sh required) >> "$log" 2>&1; then
  if [ "${JAIN_RELEASE_CI:-0}" = "1" ] && [ -f "$wt/Cargo.toml" ]; then
    cargo_receipt="$OPS_ROOT/docs/release-evidence/8.0.1/ci/${REPO}-${SHA}-cargo.json"
    (cd "$wt" && cargo metadata --locked --format-version 1 >/dev/null && \
      cargo run --locked --quiet --manifest-path "$OPS_ROOT/Cargo.toml" -- \
        run-release-cargo-commands --manifest "$CANONICAL_MANIFEST" --repo "$REPO" \
        --receipt "$cargo_receipt") >> "$log" 2>&1 || {
      tail -30 "$log" >&2
      persist_log failure
      post_check_if_apply failure || true
      say "FAIL release Cargo policy $OWNER/$REPO @ ${SHA:0:8}"
      exit 1
    }
    release_lanes=(security score contract-drift artifact-support)
    if [ "$REPO" = "jain-smartcluster" ]; then
      release_lanes+=(release)
    elif [ "$REPO" = "jain-deploy" ]; then
      release_lanes+=(container-policy image-resilience invention-export-clean atomicsoul-dry-run-test)
    fi
    for lane in "${release_lanes[@]}"; do
    if (cd "$wt" && bash scripts/ci-local.sh "$lane") >> "$log" 2>&1; then
        continue
      fi
      tail -30 "$log" >&2
      persist_log failure
      post_check_if_apply failure || true
      say "FAIL release $lane lane $OWNER/$REPO @ ${SHA:0:8}"
      exit 1
    done
  fi
  persist_log success
  if ! post_check_if_apply success; then
    say "CI passed but required status publication failed"
    exit 1
  fi
  say "PASS $OWNER/$REPO @ ${SHA:0:8}"
  exit 0
else
  rc=$?
  tail -30 "$log" >&2
  persist_log failure
  if ! post_check_if_apply failure; then
    say "required failure status publication also failed"
  fi
  say "FAIL ($rc) $OWNER/$REPO @ ${SHA:0:8}"
  exit 1
fi
