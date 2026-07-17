#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/native-runtime.sh
source "$repo_root/ops/ci/native-runtime.sh"
# shellcheck source=ops/ci/host-ci-evidence.sh
source "$repo_root/ops/ci/host-ci-evidence.sh"

mkdir -p "$repo_root/target/test-tmp"
tmp="$(mktemp -d "$repo_root/target/test-tmp/jain-native-materializer-test.XXXXXX")"
durable_root="${JAIN_TEST_DURABLE_ROOT:-$repo_root/target/test-tmp/jain-native-materializer-durable.$$}"
promotion_staging=""
promotion_small_store=""
promotion_staging_mounted=0
promotion_small_store_mounted=0
rm -rf -- "$durable_root"
mkdir -p "$durable_root"
cleanup() {
  local cleanup_rc=$?
  if [[ "$promotion_staging_mounted" == 1 ]]; then
    sudo -n /usr/bin/umount -- "$promotion_staging" >/dev/null 2>&1 || true
  fi
  if [[ "$promotion_small_store_mounted" == 1 ]]; then
    sudo -n /usr/bin/umount -- "$promotion_small_store" >/dev/null 2>&1 || true
  fi
  sudo -n rm -rf -- "$durable_root" >/dev/null 2>&1 || true
  [[ -z "${source_root:-}" ]] \
    || chmod -R u+w "$source_root" 2>/dev/null || true
  rm -rf -- "$tmp" "$durable_root"
  return "$cleanup_rc"
}
trap cleanup EXIT
source_root="$tmp/source"
run_root="$tmp/run"
vendor_root="$run_root/vendor"
authority="$tmp/native-sources.lock.json"
mkdir -p "$source_root" "$run_root"

managed_inventory='{"repositories":[
  {"kind":"control-plane","name":"jain-split-ops",
   "remote":"http://127.0.0.1:8787/git/jeryu/jain-split-ops.git",
   "required_check":"jain-split-ops/required"},
  {"kind":"family","name":"jain-core",
   "remote":"http://127.0.0.1:8787/git/jeryu/jain-core.git",
   "required_check":"jain-core/required"}
]}'
[[ "$(jain_authoritative_control_plane_remote "$managed_inventory")" == \
  'http://127.0.0.1:8787/git/jeryu/jain-split-ops.git' ]] || exit 1
[[ "$(jain_authoritative_required_check "$managed_inventory" jain-core)" == \
  'jain-core/required' ]] || exit 1
if jain_validate_native_check_mode jain-core jain-core/required \
  jain-core/required 0 2>/dev/null; then
  printf 'protected native check accepted non-release mode\n' >&2
  exit 1
fi
jain_validate_native_check_mode jain-core jain-core/required jain-core/required 1
jain_validate_native_check_mode jain-core jain-core/fast jain-core/required 0
jain_validate_native_check_mode jain-report jain-report/required jain-report/required 0
jain_native_check_requires_evidence jain-core jain-core/required jain-core/required
if jain_native_check_requires_evidence jain-core jain-core/fast jain-core/required; then
  printf 'unprotected native check unexpectedly requires release evidence\n' >&2
  exit 1
fi
if jain_verify_native_check_evidence success 1 '' '' \
  0123456789abcdef0123456789abcdef01234567 jain-core/required 2>/dev/null; then
  printf 'protected native success accepted missing evidence\n' >&2
  exit 1
fi
jain_verify_native_check_evidence failure 1 '' '' \
  0123456789abcdef0123456789abcdef01234567 jain-core/required

init_repo() {
  local learner="$1" root="$source_root/$1"
  git init --quiet "$root"
  git -C "$root" config user.name 'Native Fixture'
  git -C "$root" config user.email native-fixture@example.invalid
  case "$learner" in
    catboost)
      mkdir -p "$root/build/toolchains" "$root/catboost/libs/train_interface"
      printf 'cmake_minimum_required(VERSION 3.20)\n' >"$root/CMakeLists.txt"
      printf 'set(CMAKE_C_COMPILER clang)\n' >"$root/build/toolchains/clang.toolchain"
      printf 'add_library(catboost SHARED fixture.cpp)\n' \
        >"$root/catboost/libs/train_interface/CMakeLists.linux-x86_64.txt"
      ;;
    xgboost)
      mkdir -p "$root/include/xgboost"
      printf 'cmake_minimum_required(VERSION 3.20)\n' >"$root/CMakeLists.txt"
      printf '#pragma once\n' >"$root/include/xgboost/c_api.h"
      ;;
    lightgbm)
      mkdir -p "$root/include/LightGBM"
      printf 'cmake_minimum_required(VERSION 3.20)\n' >"$root/CMakeLists.txt"
      printf '#pragma once\n' >"$root/include/LightGBM/c_api.h"
      ;;
  esac
  git -C "$root" add .
  git -C "$root" commit --quiet -m "fixture $learner"
}

tree_manifest() {
  git -C "$1" ls-tree -r --full-tree HEAD | sha256sum | cut -d' ' -f1
}

content_digest() {
  (
    cd "$1"
    find . -type f \
      ! -path './.git' ! -path './.git/*' \
      ! -path '*/.git' ! -path '*/.git/*' \
      ! -path './.build/*' ! -path './receipts/*' \
      -print0 | LC_ALL=C sort -z | xargs -0 sha256sum
  ) | sha256sum | cut -d' ' -f1
}

for learner in catboost xgboost lightgbm; do init_repo "$learner"; done
materializer="$repo_root/ops/ci/native-materializer.sh"
materializer_sha="$(sha256sum -- "$materializer" | cut -d' ' -f1)"
jq -n --arg materializer_sha "$materializer_sha" \
  --arg cb_rev "$(git -C "$source_root/catboost" rev-parse HEAD)" \
  --arg cb_tree "$(git -C "$source_root/catboost" rev-parse 'HEAD^{tree}')" \
  --arg cb_manifest "$(tree_manifest "$source_root/catboost")" \
  --arg cb_content "$(content_digest "$source_root/catboost")" \
  --arg xgb_rev "$(git -C "$source_root/xgboost" rev-parse HEAD)" \
  --arg xgb_tree "$(git -C "$source_root/xgboost" rev-parse 'HEAD^{tree}')" \
  --arg xgb_manifest "$(tree_manifest "$source_root/xgboost")" \
  --arg xgb_content "$(content_digest "$source_root/xgboost")" \
  --arg lgb_rev "$(git -C "$source_root/lightgbm" rev-parse HEAD)" \
  --arg lgb_tree "$(git -C "$source_root/lightgbm" rev-parse 'HEAD^{tree}')" \
  --arg lgb_manifest "$(tree_manifest "$source_root/lightgbm")" \
  --arg lgb_content "$(content_digest "$source_root/lightgbm")" \
  '{schema_version:"jain.native-source-authority/v1",
    materializer:{path:"ops/ci/native-materializer.sh",sha256:$materializer_sha},
    learners:[
      {name:"catboost",revision:$cb_rev,git_tree:$cb_tree,
       tree_manifest_sha256:$cb_manifest,source_content_sha256:$cb_content,
       required_files:["CMakeLists.txt","build/toolchains/clang.toolchain",
         "catboost/libs/train_interface/CMakeLists.linux-x86_64.txt"],submodules:[]},
      {name:"xgboost",revision:$xgb_rev,git_tree:$xgb_tree,
       tree_manifest_sha256:$xgb_manifest,source_content_sha256:$xgb_content,
       required_files:["CMakeLists.txt","include/xgboost/c_api.h"],submodules:[]},
      {name:"lightgbm",revision:$lgb_rev,git_tree:$lgb_tree,
       tree_manifest_sha256:$lgb_manifest,source_content_sha256:$lgb_content,
       required_files:["CMakeLists.txt","include/LightGBM/c_api.h"],submodules:[]}
    ]}' >"$authority"

"$materializer" --authority "$authority" --source-root "$source_root" \
  --vendor-root "$vendor_root" >"$run_root/materialization.log"
[[ -s "$vendor_root/receipts/manifest.json" ]] || {
  printf 'clean exact materialization did not produce its manifest\n' >&2
  exit 1
}

printf 'dirty\n' >>"$source_root/catboost/CMakeLists.txt"
if "$materializer" --authority "$authority" --source-root "$source_root" \
  --vendor-root "$run_root/dirty-vendor" >"$tmp/dirty.log" 2>&1; then
  printf 'native materializer accepted a dirty source worktree\n' >&2
  exit 1
fi
grep -Fq 'source worktree is dirty' "$tmp/dirty.log" || {
  printf 'dirty source failure was not explicit\n' >&2
  exit 1
}
git -C "$source_root/catboost" restore CMakeLists.txt

xgb_expected="$(git -C "$source_root/xgboost" rev-parse HEAD)"
printf 'mismatch\n' >>"$source_root/xgboost/include/xgboost/c_api.h"
git -C "$source_root/xgboost" add .
git -C "$source_root/xgboost" commit --quiet -m mismatch
if "$materializer" --authority "$authority" --source-root "$source_root" \
  --vendor-root "$run_root/mismatch-vendor" >"$tmp/mismatch.log" 2>&1; then
  printf 'native materializer accepted a mismatched source revision\n' >&2
  exit 1
fi
grep -Fq 'source revision mismatch' "$tmp/mismatch.log" || {
  printf 'source revision mismatch failure was not explicit\n' >&2
  exit 1
}
git -C "$source_root/xgboost" checkout --quiet --detach "$xgb_expected"

staged_source="$tmp/staged-source"
assert_independent_primary() {
  local checkout="$1" git_dir common_dir
  git_dir="$(git -C "$checkout" rev-parse --path-format=absolute --absolute-git-dir)"
  common_dir="$(git -C "$checkout" rev-parse --path-format=absolute --git-common-dir)"
  [[ -d "$checkout/.git" && ! -L "$checkout/.git" \
    && "$git_dir" == "$checkout/.git" && "$common_dir" == "$checkout/.git" \
    && ! -e "$checkout/.git/worktrees" && ! -L "$checkout/.git/worktrees" \
    && ! -e "$checkout/.git/commondir" && ! -L "$checkout/.git/commondir" \
    && ! -e "$checkout/.git/objects/info/alternates" \
    && ! -L "$checkout/.git/objects/info/alternates" ]]
}
for learner in catboost xgboost lightgbm; do
  assert_independent_primary "$source_root/$learner" || {
    printf 'canonical native source is not an independent primary: %s\n' "$learner" >&2
    exit 1
  }
done
chmod -R a-w "$source_root"
jain_stage_native_source_worktrees "$authority" "$source_root" "$staged_source"
for learner in catboost xgboost lightgbm; do
  [[ -d "$staged_source/$learner/.git" \
    && -z "$(git -C "$staged_source/$learner" status --porcelain=v1 --untracked-files=all)" ]] \
    || {
      printf 'exact object staging produced a dirty %s worktree\n' "$learner" >&2
      exit 1
    }
done
for learner in catboost xgboost lightgbm; do
  assert_independent_primary "$source_root/$learner" || {
    printf 'native staging mutated canonical Git ownership: %s\n' "$learner" >&2
    exit 1
  }
done
chmod -R u+w "$source_root"
jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_source"
[[ ! -e "$staged_source" ]] || {
  printf 'native source worktree cleanup left its staging root behind\n' >&2
  exit 1
}

tampered_materializer="$tmp/native-materializer-tampered.sh"
cp -- "$materializer" "$tampered_materializer"
printf '# tampered\n' >>"$tampered_materializer"
chmod +x "$tampered_materializer"
if "$tampered_materializer" --authority "$authority" --source-root "$source_root" \
  --vendor-root "$run_root/tampered-vendor" >"$tmp/tampered.log" 2>&1; then
  printf 'native materializer accepted a mismatched script digest\n' >&2
  exit 1
fi
grep -Fq 'materializer digest mismatch' "$tmp/tampered.log" || {
  printf 'script digest mismatch failure was not explicit\n' >&2
  exit 1
}

control="$tmp/control"
mkdir -p "$control/ops/ci"
cp -- "$materializer" "$control/ops/ci/native-materializer.sh"
cp -- "$authority" "$control/ops/ci/native-sources.lock.json"
(
  cd "$control/ops/ci"
  sha256sum native-sources.lock.json >native-sources.lock.json.sha256
)
for path in native-runtime.sh split-host-ci.sh host-ci-integrity.sh; do
  cp -- "$repo_root/ops/ci/$path" "$control/ops/ci/$path"
done
git init --quiet "$control"
git -C "$control" config user.name 'Control Fixture'
git -C "$control" config user.email control-fixture@example.invalid
git -C "$control" add .
git -C "$control" commit --quiet -m authority
git -C "$control" branch -M main
mkdir -p "$tmp/jeryu" "$tmp/veox"
git init --quiet --bare "$tmp/jeryu/jain-split-ops.git"
git init --quiet --bare "$tmp/veox/jain-split-ops.git"
git -C "$control" remote add origin "$tmp/jeryu/jain-split-ops.git"
git -C "$control" push --quiet -u origin main
jain_extract_native_materializer \
  "$control" "$tmp/extracted" "$tmp/jeryu/jain-split-ops.git"
[[ -x "$JAIN_NATIVE_MATERIALIZER" && -s "$JAIN_NATIVE_AUTHORITY" ]] || exit 1
control_commit="$(git -C "$control" rev-parse HEAD)"
mv "$tmp/jeryu/jain-split-ops.git" "$tmp/jeryu/jain-split-ops.offline"
jain_extract_native_materializer "$control" "$tmp/extracted-offline" \
  "$tmp/jeryu/jain-split-ops.git" "$control_commit" local
[[ -x "$JAIN_NATIVE_MATERIALIZER" && -s "$JAIN_NATIVE_AUTHORITY" ]] || exit 1
mv "$tmp/jeryu/jain-split-ops.offline" "$tmp/jeryu/jain-split-ops.git"
git -C "$control" remote set-url origin "$tmp/veox/jain-split-ops.git"
if jain_extract_native_materializer "$control" "$tmp/unreviewed" \
  "$tmp/jeryu/jain-split-ops.git" 2>/dev/null; then
  printf 'exact materializer extraction accepted a divergent forge alias\n' >&2
  exit 1
fi
git -C "$control" remote set-url origin "$tmp/jeryu/jain-split-ops.git"
printf '# dirty\n' >>"$control/ops/ci/native-materializer.sh"
if jain_extract_native_materializer "$control" "$tmp/rejected" \
  "$tmp/jeryu/jain-split-ops.git" 2>/dev/null; then
  printf 'exact materializer extraction accepted a dirty reviewed script\n' >&2
  exit 1
fi
git -C "$control" restore ops/ci/native-materializer.sh

evidence_root="$durable_root/persistent-evidence"
head_sha="0123456789abcdef0123456789abcdef01234567"
control_commit="$(git -C "$control" rev-parse HEAD)"
outside_durable_root="$run_root/rejected-evidence"
if JAIN_CI_ATTEMPT_ID=ephemeral-rejected jain_persist_native_evidence \
  "$vendor_root" "$run_root/materialization.log" "$outside_durable_root" "$run_root" \
  veox jain-core "$head_sha" jain-core/required "$control_commit" \
  "$control" \
  2>/dev/null; then
  printf 'native evidence accepted an in-root ephemeral persistence root\n' >&2
  exit 1
fi
[[ ! -e "$outside_durable_root" ]]
ln -s "$run_root" "$durable_root/ephemeral-link"
if JAIN_CI_ATTEMPT_ID=symlink-rejected jain_persist_native_evidence \
  "$vendor_root" "$run_root/materialization.log" \
  "$durable_root/ephemeral-link/evidence" "$run_root" \
  veox jain-core "$head_sha" jain-core/required "$control_commit" \
  "$control" \
  2>/dev/null; then
  printf 'native evidence accepted a symlink into ephemeral CI\n' >&2
  exit 1
fi
mkdir -p "$durable_root/durable-ephemeral-run"
ln -s "$durable_root/durable-ephemeral-run" "$durable_root/durable-ephemeral-link"
if jain_resolve_durable_evidence_root \
  "$durable_root/durable-ephemeral-link/evidence" \
  "$durable_root/durable-ephemeral-run" >/dev/null 2>&1; then
  printf 'native evidence accepted a resolved path inside ephemeral CI\n' >&2
  exit 1
fi
JAIN_CI_ATTEMPT_ID=fixture jain_persist_native_evidence \
  "$vendor_root" "$run_root/materialization.log" "$evidence_root" "$run_root" \
  veox jain-core "$head_sha" jain-core/required "$control_commit" \
  "$control"
evidence_dir="$JAIN_NATIVE_EVIDENCE_DIR"
receipt_sha="$JAIN_NATIVE_EVIDENCE_SHA256"
[[ "$receipt_sha" =~ ^[0-9a-f]{64}$ ]] || exit 1
jain_verify_native_evidence "$evidence_dir" "$head_sha" jain-core/required \
  "$control" "$control_commit"
jain_verify_native_check_evidence success 1 "$evidence_dir" "$receipt_sha" \
  "$head_sha" jain-core/required "$control" "$control_commit"

# Exercise the root-only post-cgroup promotion boundary with a real, valid
# evidence ledger. The production worker deliberately has neither sudo nor
# mount authority, so this privileged integration block runs in ordinary
# exact-head review and is replaced there by the real broker boundary probes.
if [[ "${JAIN_HOST_CI_NETWORK_ISOLATED:-0}" != 1 ]]; then
promotion_staging="$tmp/native-evidence-staging"
promotion_store="$durable_root/root-evidence-store"
promotion_small_store="$durable_root/root-evidence-small-store"
promotion_symlink_store="$durable_root/root-evidence-symlink-store"
worker_uid="$(id -u)"
worker_gid="$(id -g)"
mkdir -p "$promotion_staging"
sudo -n /usr/bin/mount -t tmpfs \
  -o "nodev,nosuid,noexec,size=33554432,nr_inodes=64,mode=0700,uid=$worker_uid,gid=$worker_gid" \
  jain-native-evidence-test "$promotion_staging"
promotion_staging_mounted=1
[[ "$(stat -f -c '%T' -- "$promotion_staging")" == tmpfs \
  && "$(stat -c '%u:%g:%a' -- "$promotion_staging")" \
    == "$worker_uid:$worker_gid:700" ]] || {
  printf 'native evidence staging test did not establish its hard quota\n' >&2
  exit 1
}
sudo -n install -d -o root -g root -m 0700 \
  "$promotion_store" "$promotion_symlink_store"

reset_promotion_staging() {
  find "$promotion_staging" -xdev -mindepth 1 -delete
  promotion_evidence="$promotion_staging/veox/jain-core/$head_sha/jain-core_required/fixture"
  mkdir -p "$promotion_evidence"
  cp -a -- "$evidence_dir/." "$promotion_evidence/"
}

root_promote() {
  local store="${1:?store is required}" request_id="${2:?request ID is required}"
  sudo -n /bin/bash -ceu '
    source "$1/ops/ci/native-runtime.sh"
    source "$1/ops/ci/host-ci-evidence.sh"
    jain_host_ci_promote_native_evidence \
      "$2" "$3" "$4" "$5" veox jain-core "$6" \
      jain-core/required "$7" "$8" "$9" "${10}" "${11}"
  ' -- "$repo_root" "$promotion_staging" "$promotion_evidence" \
    "$receipt_sha" "$store" "$head_sha" "$request_id" \
    "$worker_uid" "$worker_gid" "$control" "$control_commit"
}

reset_promotion_staging
first_request_id="$(printf 'f%.0s' {1..64})"
promoted_evidence="$(root_promote "$promotion_store" "$first_request_id")"
sudo -n /bin/bash -ceu '
  source "$1/ops/ci/native-runtime.sh"
  source "$1/ops/ci/host-ci-evidence.sh"
  jain_host_ci_verify_promoted_evidence "$2"
' -- "$repo_root" "$promoted_evidence"
if { printf 'worker tamper\n' >>"$promoted_evidence/materialization.log"; } \
    2>/dev/null; then
  printf 'worker could mutate root-promoted native evidence\n' >&2
  exit 1
fi

# A symlink anywhere in the worker staging tree is rejected before copying.
reset_promotion_staging
rm -- "$promotion_evidence/materialization.log"
ln -s /etc/passwd "$promotion_evidence/materialization.log"
if root_promote "$promotion_store" "$(printf 'e%.0s' {1..64})" \
    >"$tmp/promotion-symlink.log" 2>&1; then
  printf 'native evidence promotion accepted a staging symlink\n' >&2
  exit 1
fi

# Per-file and total limits are checked independently of receipt checksums.
reset_promotion_staging
truncate -s "$((JAIN_NATIVE_EVIDENCE_MAX_FILE_BYTES + 1))" \
  "$promotion_evidence/materialization.log"
if root_promote "$promotion_store" "$(printf 'd%.0s' {1..64})" \
    >"$tmp/promotion-oversize.log" 2>&1; then
  printf 'native evidence promotion accepted an oversized artifact\n' >&2
  exit 1
fi

# The tmpfs quota is a kernel-enforced ceiling, not only a post-write check.
reset_promotion_staging
if dd if=/dev/zero of="$promotion_staging/quota-fill" bs=1048576 count=40 \
    status=none 2>"$tmp/promotion-quota-fill.log"; then
  printf 'native evidence staging exceeded its 32 MiB hard quota\n' >&2
  exit 1
fi
quota_fill_size="$(stat -c '%s' -- "$promotion_staging/quota-fill")"
(( quota_fill_size > 0 && quota_fill_size < 40 * 1048576 )) || {
  printf 'native evidence staging quota did not fail closed\n' >&2
  exit 1
}
rm -- "$promotion_staging/quota-fill"

# A symlinked durable hierarchy component cannot redirect root promotion.
reset_promotion_staging
sudo -n ln -s "$tmp" "$promotion_symlink_store/veox"
if root_promote "$promotion_symlink_store" "$(printf 'c%.0s' {1..64})" \
    >"$tmp/promotion-store-symlink.log" 2>&1; then
  printf 'native evidence promotion accepted a durable-store symlink\n' >&2
  exit 1
fi

# Preserve at least 1 GiB after promotion. A deliberately small durable
# filesystem deterministically exercises the low-space/fill rejection.
sudo -n install -d -o root -g root -m 0700 "$promotion_small_store"
sudo -n /usr/bin/mount -t tmpfs \
  -o 'nodev,nosuid,noexec,size=67108864,nr_inodes=128,mode=0700,uid=0,gid=0' \
  jain-native-evidence-small-store "$promotion_small_store"
promotion_small_store_mounted=1
if root_promote "$promotion_small_store" "$(printf 'b%.0s' {1..64})" \
    >"$tmp/promotion-free-space.log" 2>&1; then
  printf 'native evidence promotion ignored its free-space reserve\n' >&2
  exit 1
fi
grep -Fq 'native evidence store free-space guard failed' \
  "$tmp/promotion-free-space.log" || {
  printf 'native evidence low-space rejection was not explicit\n' >&2
  exit 1
}

# Keep only the newest eight immutable attempts for a check.
for attempt in $(seq 1 10); do
  request_id="$(printf '%064x' "$attempt")"
  root_promote "$promotion_store" "$request_id" >/dev/null
done
retention_parent="$promotion_store/veox/jain-core/jain-core_required"
retained_count="$(sudo -n find "$retention_parent" -mindepth 1 -maxdepth 1 \
  -type d -regextype posix-extended -regex '.*/[0-9a-f]{64}' | wc -l)"
[[ "$retained_count" == 8 ]] || {
  printf 'native evidence retention did not preserve exactly the newest eight attempts\n' >&2
  exit 1
}
if sudo -n test -e "$retention_parent/$(printf '%064x' 1)" \
  || sudo -n test -e "$retention_parent/$(printf '%064x' 2)"; then
  printf 'native evidence retention preserved an expired attempt\n' >&2
  exit 1
fi
latest_evidence="$retention_parent/$(printf '%064x' 10)"
[[ "$(sudo -n stat -c '%u:%g:%a' -- "$latest_evidence")" == '0:0:500' ]]

sudo -n /usr/bin/umount -- "$promotion_small_store"
promotion_small_store_mounted=0
sudo -n /usr/bin/umount -- "$promotion_staging"
promotion_staging_mounted=0
else
  printf 'native materializer privileged promotion covered by isolated broker boundary\n'
fi
rm -rf -- "$run_root"
jain_verify_native_evidence "$evidence_dir" "$head_sha" jain-core/required \
  "$control" "$control_commit" || {
  printf 'native evidence did not survive ephemeral cleanup\n' >&2
  exit 1
}

# A receipt cannot relabel orchestration from one reviewed commit as another,
# even when every mutable sidecar and receipt digest is recomputed.
mixed_commit_evidence="$tmp/mixed-commit-evidence"
cp -a -- "$evidence_dir" "$mixed_commit_evidence"
printf '# successor commit\n' >>"$control/ops/ci/native-runtime.sh"
git -C "$control" add ops/ci/native-runtime.sh
git -C "$control" commit --quiet -m successor
successor_commit="$(git -C "$control" rev-parse HEAD)"
jq --arg commit "$successor_commit" '.control_plane_commit = $commit' \
  "$mixed_commit_evidence/receipt.json" >"$mixed_commit_evidence/receipt.json.new"
mv -- "$mixed_commit_evidence/receipt.json.new" \
  "$mixed_commit_evidence/receipt.json"
jain_write_sha256_sidecar "$mixed_commit_evidence/receipt.json"
if jain_verify_native_evidence "$mixed_commit_evidence" "$head_sha" \
  jain-core/required "$control" "$successor_commit" 2>/dev/null; then
  printf 'native evidence accepted orchestration from a different commit\n' >&2
  exit 1
fi

receipt_tamper="$tmp/receipt-tamper"
cp -a -- "$evidence_dir" "$receipt_tamper"
jq '.recorded_at = "tampered"' "$receipt_tamper/receipt.json" \
  >"$receipt_tamper/receipt.json.new"
mv -- "$receipt_tamper/receipt.json.new" "$receipt_tamper/receipt.json"
jain_write_sha256_sidecar "$receipt_tamper/receipt.json"
if jain_verify_native_evidence_binding "$receipt_tamper" "$receipt_sha" \
  "$head_sha" jain-core/required "$control" "$control_commit" 2>/dev/null; then
  printf 'native evidence binding accepted a re-checksummed receipt tamper\n' >&2
  exit 1
fi
printf 'tamper\n' >>"$evidence_dir/materialization.log"
jain_write_sha256_sidecar "$evidence_dir/materialization.log"
if jain_verify_native_evidence "$evidence_dir" "$head_sha" jain-core/required \
  "$control" "$control_commit" 2>/dev/null; then
  printf 'native evidence verification accepted a re-checksummed log tamper\n' >&2
  exit 1
fi

grep -Fq 'managed-repos --manifest "$CANONICAL_MANIFEST" --json' \
  "$repo_root/ops/ci/split-host-ci.sh" || {
  printf 'host CI does not derive policy from the authority manifest\n' >&2
  exit 1
}
if grep -Fq 'git/veox/jain-split-ops.git' "$repo_root/ops/ci/split-host-ci.sh"; then
  printf 'host CI still embeds the divergent control-plane alias\n' >&2
  exit 1
fi
grep -Fq 'post_check failure' "$repo_root/ops/ci/split-host-ci.sh" || {
  printf 'host CI native setup failures do not publish failure status\n' >&2
  exit 1
}

printf 'native materializer authority/evidence contract ok\n'
