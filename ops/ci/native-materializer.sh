#!/usr/bin/env bash
# Materialize exact native learner sources for detached release CI.
#
# This script is executed from the reviewed jain-split-ops commit, never from a
# mutable product repository. The authority document pins this script and every
# source/submodule identity used to produce the private native vendor tree.
set -euo pipefail

die() {
  printf 'native-materializer: %s\n' "$*" >&2
  exit 1
}

usage() {
  printf 'usage: %s --authority FILE --source-root DIR --vendor-root DIR\n' \
    "${0##*/}" >&2
  exit 2
}

authority=""
source_root=""
vendor_root=""
while (($#)); do
  case "$1" in
    --authority) authority="${2:-}"; shift 2 ;;
    --source-root) source_root="${2:-}"; shift 2 ;;
    --vendor-root) vendor_root="${2:-}"; shift 2 ;;
    *) usage ;;
  esac
done
[[ -n "$authority" && -n "$source_root" && -n "$vendor_root" ]] || usage

for tool in git jq sha256sum find sort xargs rsync; do
  command -v "$tool" >/dev/null 2>&1 || die "required command not found: $tool"
done
[[ "$authority" = /* && "$source_root" = /* && "$vendor_root" = /* ]] \
  || die "authority, source root, and vendor root must be absolute"
[[ -f "$authority" ]] || die "authority document does not exist: $authority"
[[ -d "$source_root" ]] || die "source root does not exist: $source_root"

script_path="$(readlink -f "${BASH_SOURCE[0]}")"
script_digest="$(sha256sum -- "$script_path" | cut -d' ' -f1)"
expected_script_digest="$(jq -er \
  'select(.schema_version == "jain.native-source-authority/v1")
   | .materializer.sha256
   | select(test("^[0-9a-f]{64}$"))' "$authority")" \
  || die "invalid native source authority document"
[[ "$script_digest" == "$expected_script_digest" ]] || die \
  "materializer digest mismatch: expected $expected_script_digest, got $script_digest"

mapfile -t learners < <(jq -er \
  '.learners | map(.name) as $names
   | select(($names | sort) == ["catboost", "lightgbm", "xgboost"])
   | $names[]' "$authority")
[[ "${#learners[@]}" -eq 3 ]] || die "authority must name each native learner exactly once"

tree_manifest_digest() {
  local repo="$1" revision="$2"
  git -C "$repo" ls-tree -r --full-tree "$revision" | sha256sum | cut -d' ' -f1
}

content_digest() {
  local root="$1"
  (
    cd "$root"
    find . -type f \
      ! -path './.git' ! -path './.git/*' \
      ! -path '*/.git' ! -path '*/.git/*' \
      ! -path './.build/*' ! -path './receipts/*' \
      -print0 | LC_ALL=C sort -z | xargs -0 sha256sum
  ) | sha256sum | cut -d' ' -f1
}

validate_git_tree() {
  local learner="$1" repo="$2" revision="$3" tree="$4" manifest_digest="$5"
  local actual_revision actual_tree actual_manifest dirty
  [[ -d "$repo" ]] || die "$learner source is missing: $repo"
  [[ "$(git -C "$repo" rev-parse --is-inside-work-tree 2>/dev/null || true)" == true ]] \
    || die "$learner source is not a Git worktree: $repo"
  actual_revision="$(git -C "$repo" rev-parse --verify 'HEAD^{commit}' 2>/dev/null || true)"
  [[ "$actual_revision" == "$revision" ]] || die \
    "$learner source revision mismatch: expected $revision, got ${actual_revision:-unresolved}"
  actual_tree="$(git -C "$repo" rev-parse --verify 'HEAD^{tree}' 2>/dev/null || true)"
  [[ "$actual_tree" == "$tree" ]] || die \
    "$learner source tree mismatch: expected $tree, got ${actual_tree:-unresolved}"
  actual_manifest="$(tree_manifest_digest "$repo" HEAD)"
  [[ "$actual_manifest" == "$manifest_digest" ]] || die \
    "$learner source manifest mismatch: expected $manifest_digest, got $actual_manifest"
  dirty="$(git -C "$repo" status --porcelain=v1 --untracked-files=all --ignore-submodules=none)"
  [[ -z "$dirty" ]] || die "$learner source worktree is dirty: $repo"
}

required_file_count() {
  jq -er --arg learner "$1" \
    '.learners[] | select(.name == $learner) | .required_files | length | select(. > 0)' \
    "$authority"
}

validate_learner() {
  local learner="$1"
  local source="$source_root/$learner"
  local revision tree manifest_digest content_sha count index relative sub_count sub_path
  revision="$(jq -er --arg learner "$learner" \
    '.learners[] | select(.name == $learner) | .revision | select(test("^[0-9a-f]{40}$"))' \
    "$authority")"
  tree="$(jq -er --arg learner "$learner" \
    '.learners[] | select(.name == $learner) | .git_tree | select(test("^[0-9a-f]{40}$"))' \
    "$authority")"
  manifest_digest="$(jq -er --arg learner "$learner" \
    '.learners[] | select(.name == $learner) | .tree_manifest_sha256
     | select(test("^[0-9a-f]{64}$"))' "$authority")"
  validate_git_tree "$learner" "$source" "$revision" "$tree" "$manifest_digest"
  content_sha="$(jq -er --arg learner "$learner" \
    '.learners[] | select(.name == $learner) | .source_content_sha256
     | select(test("^[0-9a-f]{64}$"))' "$authority")"
  [[ "$(content_digest "$source")" == "$content_sha" ]] || die \
    "$learner source content digest does not match authority"

  count="$(required_file_count "$learner")"
  for ((index = 0; index < count; index++)); do
    relative="$(jq -er --arg learner "$learner" --argjson index "$index" \
      '.learners[] | select(.name == $learner) | .required_files[$index]
       | select(type == "string" and length > 0 and (startswith("/") | not)
         and (contains("..") | not))' "$authority")" \
      || die "$learner authority contains an invalid required path"
    [[ -s "$source/$relative" ]] || die \
      "$learner source is missing required file: $source/$relative"
  done

  sub_count="$(jq -er --arg learner "$learner" \
    '.learners[] | select(.name == $learner) | (.submodules // []) | length' "$authority")"
  for ((index = 0; index < sub_count; index++)); do
    sub_path="$(jq -er --arg learner "$learner" --argjson index "$index" \
      '.learners[] | select(.name == $learner) | .submodules[$index].path
       | select(type == "string" and length > 0 and (startswith("/") | not)
         and (contains("..") | not))' "$authority")" \
      || die "$learner authority contains an invalid submodule path"
    revision="$(jq -er --arg learner "$learner" --argjson index "$index" \
      '.learners[] | select(.name == $learner) | .submodules[$index].revision
       | select(test("^[0-9a-f]{40}$"))' "$authority")"
    tree="$(jq -er --arg learner "$learner" --argjson index "$index" \
      '.learners[] | select(.name == $learner) | .submodules[$index].git_tree
       | select(test("^[0-9a-f]{40}$"))' "$authority")"
    manifest_digest="$(jq -er --arg learner "$learner" --argjson index "$index" \
      '.learners[] | select(.name == $learner) | .submodules[$index].tree_manifest_sha256
       | select(test("^[0-9a-f]{64}$"))' "$authority")"
    validate_git_tree "$learner submodule $sub_path" "$source/$sub_path" \
      "$revision" "$tree" "$manifest_digest"
  done
}

prune_native_sources() {
  local root="$1" path
  for path in \
    "$root/xgboost/R-package" "$root/xgboost/python-package" \
    "$root/xgboost/jvm-packages" "$root/xgboost/demo" "$root/xgboost/doc" \
    "$root/xgboost/tests" "$root/lightgbm/R-package" \
    "$root/lightgbm/python-package" "$root/lightgbm/swig" \
    "$root/lightgbm/examples" "$root/lightgbm/docs" "$root/lightgbm/tests" \
    "$root/lightgbm/docker" "$root/lightgbm/windows" \
    "$root/lightgbm/build-python.sh" "$root/lightgbm/build_r.R" \
    "$root/catboost/catboost/R-package" "$root/catboost/catboost/python-package" \
    "$root/catboost/catboost/jvm-packages" "$root/catboost/catboost/dotnet" \
    "$root/catboost/catboost/node-package" "$root/catboost/catboost/spark" \
    "$root/catboost/catboost/pytest" "$root/catboost/catboost/docs" \
    "$root/catboost/catboost/tutorials" "$root/catboost/catboost/benchmarks" \
    "$root/catboost/catboost/docker" "$root/catboost/catboost/debian" \
    "$root/AutogluonModels" "$root/catboost_info" \
    "$root/lightgbm/lib_lightgbm.so" "$root/xgboost/lib/libxgboost.so" \
    "$root/xgboost/lib/libxgboost4j.so"; do
    rm -rf -- "$path"
  done
  rm -f -- \
    "$root/catboost/CMakeUserPresets.json" \
    "$root/catboost/catboost/libs/train_interface/catboost_c_api.cpp" \
    "$root/catboost/catboost/libs/train_interface/catboost_c_api.h" \
    "$root/catboost/catboost/libs/train_interface/cb_shim.cpp" \
    "$root/catboost/catboost/libs/train_interface/cb_shim.h"
  rm -rf -- "$root/catboost/library/python" "$root/catboost/contrib/libs/python"
  if find "$root/catboost" \( -path '*/library/python' -o -path '*/contrib/libs/python' \) \
    -print -quit | grep -q .; then
    die "catboost vendor still contains Python library payloads after pruning"
  fi
}

for learner in "${learners[@]}"; do
  validate_learner "$learner"
done

vendor_parent="$(dirname "$vendor_root")"
vendor_name="$(basename "$vendor_root")"
mkdir -p "$vendor_parent"
vendor_parent="$(cd "$vendor_parent" && pwd)"
vendor_root="$vendor_parent/$vendor_name"
case "$vendor_root/" in
  "$source_root/"*) die "destination must not be inside the native source root" ;;
esac

staging="$(mktemp -d "$vendor_parent/.native-vendor-staging.XXXXXX")"
backup=""
cleanup() {
  [[ -z "$staging" || ! -e "$staging" ]] || rm -rf -- "$staging"
  [[ -z "$backup" || ! -e "$backup" ]] || rm -rf -- "$backup"
}
trap cleanup EXIT

for learner in "${learners[@]}"; do
  mkdir -p "$staging/$learner"
  rsync -a --delete --exclude='.git' "$source_root/$learner/" "$staging/$learner/"
  expected_content="$(jq -er --arg learner "$learner" \
    '.learners[] | select(.name == $learner) | .source_content_sha256' "$authority")"
  [[ "$(content_digest "$staging/$learner")" == "$expected_content" ]] || die \
    "$learner copied source content does not match authority"
done
prune_native_sources "$staging"

mkdir -p "$staging/receipts"
for learner in "${learners[@]}"; do
  revision="$(jq -er --arg learner "$learner" \
    '.learners[] | select(.name == $learner) | .revision' "$authority")"
  source_digest="$(jq -er --arg learner "$learner" \
    '.learners[] | select(.name == $learner) | .source_content_sha256' "$authority")"
  vendor_digest="$(content_digest "$staging/$learner")"
  jq -n --arg learner "$learner" --arg source "$source_root/$learner" \
    --arg dest "$vendor_root/$learner" --arg revision "$revision" \
    --arg source_digest_sha256 "$source_digest" \
    --arg vendor_digest_sha256 "$vendor_digest" \
    '{schema_version:"jain.native-vendor/v1",learner:$learner,source:$source,dest:$dest,
      revision:$revision,source_digest_sha256:$source_digest_sha256,
      vendor_digest_sha256:$vendor_digest_sha256,pruned:true}' \
    >"$staging/receipts/$learner.json"
done

authority_digest="$(sha256sum -- "$authority" | cut -d' ' -f1)"
jq -n --arg source_root "$source_root" --arg vendor_root "$vendor_root" \
  --arg materializer "jain-split-ops/ops/ci/native-materializer.sh" \
  --arg authority_sha256 "$authority_digest" --arg materializer_sha256 "$script_digest" \
  --slurpfile catboost "$staging/receipts/catboost.json" \
  --slurpfile xgboost "$staging/receipts/xgboost.json" \
  --slurpfile lightgbm "$staging/receipts/lightgbm.json" \
  '{schema_version:"jain.native-vendor-manifest/v1",source_root:$source_root,
    vendor_root:$vendor_root,materializer:$materializer,authority_sha256:$authority_sha256,
    materializer_sha256:$materializer_sha256,atomic_install:true,
    learners:{catboost:$catboost[0],xgboost:$xgboost[0],lightgbm:$lightgbm[0]}}' \
  >"$staging/receipts/manifest.json"

if [[ -e "$vendor_root" || -L "$vendor_root" ]]; then
  backup="$(mktemp -d "$vendor_parent/.native-vendor-backup.XXXXXX")"
  rmdir "$backup"
  mv -- "$vendor_root" "$backup"
fi
if mv -- "$staging" "$vendor_root"; then
  staging=""
else
  if [[ -n "$backup" && -e "$backup" ]]; then
    mv -- "$backup" "$vendor_root"
    backup=""
  fi
  die "atomic install failed for $vendor_root"
fi
if [[ -n "$backup" && -e "$backup" ]]; then
  rm -rf -- "$backup"
  backup=""
fi

printf 'native vendor ready: %s\n' "$vendor_root"
printf 'export JAIN_VENDOR_ROOT=%q\n' "$vendor_root"
