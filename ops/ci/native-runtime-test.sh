#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/native-runtime.sh
source "$repo_root/ops/ci/native-runtime.sh"

tmp="$(mktemp -d /tmp/jain-native-runtime-test.XXXXXX)"
trap 'rm -rf "$tmp"' EXIT

vendor_root="$tmp/native-vendor"
mkdir -p "$vendor_root/catboost" "$vendor_root/xgboost" "$vendor_root/lightgbm"
mapfile -t build_dirs < <(jain_native_build_dirs "$vendor_root")
mapfile -t runtime_dirs < <(jain_native_runtime_dirs "$vendor_root")

missing_vendor="$tmp/missing-vendor"
sentinel="$tmp/unchanged"
export JAIN_VENDOR_ROOT="$sentinel"
export LD_LIBRARY_PATH="$sentinel"
if jain_prepare_native_runtime "$missing_vendor" 2>/dev/null; then
  printf 'native runtime preparation accepted a missing vendor root\n' >&2
  exit 1
fi
[[ "$JAIN_VENDOR_ROOT" == "$sentinel" && "$LD_LIBRARY_PATH" == "$sentinel" ]] || {
  printf 'failed native runtime preparation mutated its environment\n' >&2
  exit 1
}

prior_path="$tmp/prior-one:$tmp/prior-two"
export LD_LIBRARY_PATH="$prior_path"
jain_prepare_native_runtime "$vendor_root"
expected="$(IFS=:; printf '%s' "${runtime_dirs[*]}"):$prior_path"
[[ "$LD_LIBRARY_PATH" == "$expected" ]] || {
  printf 'native runtime path order mismatch: expected %s, got %s\n' \
    "$expected" "$LD_LIBRARY_PATH" >&2
  exit 1
}
[[ "$JAIN_VENDOR_ROOT" == "$vendor_root" ]] || exit 1
[[ "$CATBOOST_BUILD_DIR" == "${build_dirs[0]}" ]] || exit 1
[[ "$XGBOOST_BUILD_DIR" == "${build_dirs[1]}" ]] || exit 1
[[ "$LIGHTGBM_BUILD_DIR" == "${build_dirs[2]}" ]] || exit 1
for dir in "${build_dirs[@]}"; do
  [[ -d "$dir" ]] || {
    printf 'native build destination vanished: %s\n' "$dir" >&2
    exit 1
  }
done
[[ ! -d "${runtime_dirs[0]}" ]] || {
  printf 'native preparation created CatBoost final output directory before build\n' >&2
  exit 1
}

if jain_verify_native_libraries "$vendor_root" catboost xgboost lightgbm 2>/dev/null; then
  printf 'native verification accepted missing learner libraries\n' >&2
  exit 1
fi
for learner in catboost xgboost lightgbm; do
  library="$(jain_native_library_path "$vendor_root" "$learner")"
  mkdir -p "$(dirname "$library")"
  cp /bin/true "$library"
done
jain_verify_native_libraries "$vendor_root" catboost xgboost lightgbm

xgboost_library="$(jain_native_library_path "$vendor_root" xgboost)"
: >"$xgboost_library"
if jain_verify_native_libraries "$vendor_root" xgboost 2>/dev/null; then
  printf 'native verification accepted an empty learner library\n' >&2
  exit 1
fi
cp /bin/true "$xgboost_library"

release_dir="$tmp/cargo-target/release"
mkdir -p "$release_dir"
cp /bin/true "$release_dir/native-smoke"
jain_verify_linked_binaries "$release_dir"

ldd() {
  printf 'libnative-missing.so => not found\n'
}
if jain_verify_linked_binaries "$release_dir" 2>/dev/null; then
  printf 'linked-binary verification accepted an unresolved dependency\n' >&2
  exit 1
fi
unset -f ldd

mapfile -t core_learners < <(jain_native_learners_for_repo jain-core)
[[ "${core_learners[*]}" == "catboost xgboost lightgbm" ]] || {
  printf 'Core native learner mapping drifted\n' >&2
  exit 1
}
mapfile -t report_learners < <(jain_native_learners_for_repo jain-report)
[[ "${#report_learners[@]}" -eq 0 ]] || exit 1

printf 'native runtime build/link contract ok\n'
