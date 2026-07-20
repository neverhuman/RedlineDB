#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/native-runtime.sh
source "$repo_root/ops/ci/native-runtime.sh"

tmp="$(mktemp -d /tmp/jain-native-runtime-test.XXXXXX)"
cleanup() {
  chmod -R u+w "$tmp" 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup EXIT

fixture_stage="$tmp/native-build-tools-stage"
mkdir -p "$fixture_stage/bin" "$fixture_stage/share/cmake-4.3"
for tool in cmake ninja ragel yasm; do
  case "$tool" in
    cmake) version='cmake version 4.3.2' ;;
    ninja) version='1.13.0.git.kitware.jobserver-pipe-1' ;;
    ragel) version='Ragel State Machine Compiler version 6.10 March 2017' ;;
    yasm) version='yasm 1.3.0' ;;
  esac
  printf '#!/usr/bin/env bash\nprintf "%%s\\n" "%s"\n' "$version" \
    >"$fixture_stage/bin/$tool"
done
printf 'fixture module\n' >"$fixture_stage/share/cmake-4.3/Fixture.cmake"
find "$fixture_stage" -type d -exec chmod 0555 {} +
find "$fixture_stage" -type f -exec chmod 0444 {} +
chmod 0555 "$fixture_stage"/bin/*
fixture_inventory="$({
  while IFS= read -r relative; do
    path="$fixture_stage/$relative"
    printf '%s\t%s\t%s\t%s\n' "$relative" \
      "$(stat -c %a -- "$path")" "$(stat -c %s -- "$path")" \
      "$(sha256sum -- "$path" | cut -d' ' -f1)"
  done < <(LC_ALL=C find "$fixture_stage" -type f -printf '%P\n' | LC_ALL=C sort)
} | sha256sum | cut -d' ' -f1)"
fixture_root="$tmp/$fixture_inventory"
mv -- "$fixture_stage" "$fixture_root"
fixture_authority="$tmp/native-build-tools.lock.json"
jq -n --arg inventory "$fixture_inventory" --arg root \
  "/var/lib/jain-host-ci/native-build-tools/$fixture_inventory" \
  --arg cmake_sha "$(sha256sum "$fixture_root/bin/cmake" | cut -d' ' -f1)" \
  --arg ninja_sha "$(sha256sum "$fixture_root/bin/ninja" | cut -d' ' -f1)" \
  --arg ragel_sha "$(sha256sum "$fixture_root/bin/ragel" | cut -d' ' -f1)" \
  --arg yasm_sha "$(sha256sum "$fixture_root/bin/yasm" | cut -d' ' -f1)" \
  --argjson cmake_size "$(stat -c %s "$fixture_root/bin/cmake")" \
  --argjson ninja_size "$(stat -c %s "$fixture_root/bin/ninja")" \
  --argjson ragel_size "$(stat -c %s "$fixture_root/bin/ragel")" \
  --argjson yasm_size "$(stat -c %s "$fixture_root/bin/yasm")" \
  '{schema_version:"jain.native-build-tools/v1",bundle_root:$root,
    inventory_sha256:$inventory,file_count:5,
    tools:{
      cmake:{path:"bin/cmake",mode:"555",size:$cmake_size,
        sha256:$cmake_sha,version:"cmake version 4.3.2"},
      ninja:{path:"bin/ninja",mode:"555",size:$ninja_size,
        sha256:$ninja_sha,version:"1.13.0.git.kitware.jobserver-pipe-1"},
      ragel:{path:"bin/ragel",mode:"555",size:$ragel_size,
        sha256:$ragel_sha,
        version:"Ragel State Machine Compiler version 6.10 March 2017"},
      yasm:{path:"bin/yasm",mode:"555",size:$yasm_size,
        sha256:$yasm_sha,version:"yasm 1.3.0"}}}' >"$fixture_authority"

jain_validate_native_build_tools "$fixture_authority" "$fixture_root" content
if jain_validate_native_build_tools \
  "$fixture_authority" "$fixture_root" root 2>/dev/null; then
  printf 'native build-tool validator accepted non-root fixture ownership\n' >&2
  exit 1
fi
prior_path="$PATH"
jain_activate_native_build_tools "$fixture_authority" "$fixture_root"
[[ "$CMAKE" == "$fixture_root/bin/cmake" \
  && "$NINJA" == "$fixture_root/bin/ninja" \
  && "$CMAKE_MAKE_PROGRAM" == "$fixture_root/bin/ninja" \
  && "$PATH" == "$fixture_root/bin:$prior_path" ]] || exit 1
PATH="$prior_path"

chmod 0644 "$fixture_root/bin/ninja"
if jain_validate_native_build_tools \
  "$fixture_authority" "$fixture_root" content 2>/dev/null; then
  printf 'native build-tool validator accepted a mutable tool\n' >&2
  exit 1
fi
chmod 0555 "$fixture_root/bin/ninja"
chmod 0755 "$fixture_root/share/cmake-4.3"
ln -s Fixture.cmake "$fixture_root/share/cmake-4.3/linked.cmake"
chmod 0555 "$fixture_root/share/cmake-4.3"
if jain_validate_native_build_tools \
  "$fixture_authority" "$fixture_root" content 2>/dev/null; then
  printf 'native build-tool validator accepted a symlink\n' >&2
  exit 1
fi
chmod 0755 "$fixture_root/share/cmake-4.3"
rm -- "$fixture_root/share/cmake-4.3/linked.cmake"
ln "$fixture_root/share/cmake-4.3/Fixture.cmake" \
  "$fixture_root/share/cmake-4.3/hardlink.cmake"
chmod 0555 "$fixture_root/share/cmake-4.3"
if jain_validate_native_build_tools \
  "$fixture_authority" "$fixture_root" content 2>/dev/null; then
  printf 'native build-tool validator accepted a hardlink\n' >&2
  exit 1
fi
chmod 0755 "$fixture_root/share/cmake-4.3"
rm -- "$fixture_root/share/cmake-4.3/hardlink.cmake"
chmod 0555 "$fixture_root/share/cmake-4.3"
chmod 0644 "$fixture_root/share/cmake-4.3/Fixture.cmake"
printf 'tampered module\n' >"$fixture_root/share/cmake-4.3/Fixture.cmake"
chmod 0444 "$fixture_root/share/cmake-4.3/Fixture.cmake"
if jain_validate_native_build_tools \
  "$fixture_authority" "$fixture_root" content 2>/dev/null; then
  printf 'native build-tool validator accepted tampered module bytes\n' >&2
  exit 1
fi
chmod 0644 "$fixture_root/share/cmake-4.3/Fixture.cmake"
printf 'fixture module\n' >"$fixture_root/share/cmake-4.3/Fixture.cmake"
chmod 0444 "$fixture_root/share/cmake-4.3/Fixture.cmake"
jq '.tools.cmake.version = "cmake version 0.0.0"' "$fixture_authority" \
  >"$tmp/wrong-version.json"
if jain_validate_native_build_tools \
  "$tmp/wrong-version.json" "$fixture_root" content 2>/dev/null; then
  printf 'native build-tool validator accepted a wrong version\n' >&2
  exit 1
fi
jq '.unreviewed = true' "$fixture_authority" >"$tmp/unknown-key.json"
if jain_validate_native_build_tools \
  "$tmp/unknown-key.json" "$fixture_root" content 2>/dev/null; then
  printf 'native build-tool validator accepted an unknown authority field\n' >&2
  exit 1
fi

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
