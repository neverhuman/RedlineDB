#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/native-runtime.sh
source "$repo_root/ops/ci/native-runtime.sh"

tmp="$(mktemp -d /tmp/jain-native-runtime-test.XXXXXX)"
trap 'rm -rf "$tmp"' EXIT

vendor_root="$tmp/native-vendor"
mapfile -t runtime_dirs < <(jain_native_runtime_dirs "$vendor_root")
mkdir -p "${runtime_dirs[@]}"

prior_path="$tmp/prior-one:$tmp/prior-two"
export LD_LIBRARY_PATH="$prior_path"
jain_export_native_runtime_path "$vendor_root"
expected="$(IFS=:; printf '%s' "${runtime_dirs[*]}"):$prior_path"
[[ "$LD_LIBRARY_PATH" == "$expected" ]] || {
  printf 'native runtime path order mismatch: expected %s, got %s\n' \
    "$expected" "$LD_LIBRARY_PATH" >&2
  exit 1
}
for dir in "${runtime_dirs[@]}"; do
  [[ -d "$dir" ]] || {
    printf 'native runtime test directory vanished: %s\n' "$dir" >&2
    exit 1
  }
done

rm -rf "${runtime_dirs[1]}"
sentinel="$tmp/unchanged"
export LD_LIBRARY_PATH="$sentinel"
if jain_export_native_runtime_path "$vendor_root" 2>/dev/null; then
  printf 'native runtime export accepted a missing learner directory\n' >&2
  exit 1
fi
[[ "$LD_LIBRARY_PATH" == "$sentinel" ]] || {
  printf 'failed native runtime export mutated LD_LIBRARY_PATH\n' >&2
  exit 1
}

printf 'native runtime path contract ok\n'
