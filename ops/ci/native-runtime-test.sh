#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/native-runtime.sh
source "$repo_root/ops/ci/native-runtime.sh"

validate_fixture_under_pipefail() {
  local authority="${1:?authority is required}"
  local root="${2:?root is required}"
  bash -c 'set -euo pipefail
source "$1"
jain_validate_native_build_tools "$2" "$3" content' -- \
    "$repo_root/ops/ci/native-runtime.sh" "$authority" "$root"
}

tmp="$(mktemp -d /tmp/jain-native-runtime-test.XXXXXX)"
cleanup() {
  chmod -R u+w "$tmp" 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup EXIT

fixture_stage="$tmp/native-build-tools-stage"
mkdir -p "$fixture_stage/bin" \
  "$fixture_stage/share/cmake-4.3/Help/generator"
for tool in cmake lld ninja ragel yasm; do
  relative="$tool"
  case "$tool" in
    cmake) version='cmake version 4.3.2' ;;
    lld)
      relative=ld.lld
      version='Ubuntu LLD 21.1.8 (compatible with GNU linkers)'
      ;;
    ninja) version='1.13.0.git.kitware.jobserver-pipe-1' ;;
    ragel) version='Ragel State Machine Compiler version 6.10 March 2017' ;;
    yasm) version='yasm 1.3.0' ;;
  esac
  if [[ "$tool" == lld ]]; then
    printf '%s\n' '#!/usr/bin/env bash' \
      'if [[ "${1-}" == --version ]]; then' \
      "  printf '%s\\n' '$version'" \
      '  exit 0' \
      'fi' \
      'exec /usr/bin/ld "$@"' >"$fixture_stage/bin/$relative"
  else
    printf '#!/usr/bin/env bash\nprintf "%%s\\n" "%s"\n' "$version" \
      >"$fixture_stage/bin/$relative"
  fi
done
printf 'fixture module\n' >"$fixture_stage/share/cmake-4.3/Fixture.cmake"
printf 'space-bearing fixture\n' \
  >"$fixture_stage/share/cmake-4.3/Help/generator/Borland Makefiles.rst"
printf 'else fixture\n' >"$fixture_stage/share/cmake-4.3/Help/else.rst"
printf 'elseif fixture\n' >"$fixture_stage/share/cmake-4.3/Help/elseif.rst"
find "$fixture_stage" -type d -exec chmod 0555 {} +
find "$fixture_stage" -type f -exec chmod 0444 {} +
chmod 0555 "$fixture_stage"/bin/*
fixture_authority="$tmp/native-build-tools.lock.json"
"$repo_root/ops/ci/native-build-tools-authority.sh" "$fixture_stage" \
  >"$fixture_authority"
fixture_inventory="$(jq -er '.inventory_sha256' "$fixture_authority")"
fixture_root="$tmp/$fixture_inventory"
mv -- "$fixture_stage" "$fixture_root"

for valid_path in \
  'bin/cmake' \
  'share/cmake-4.3/Help/generator/Borland Makefiles.rst' \
  'share/A + B/file-name_1.2'; do
  jain_native_relative_path_is_canonical "$valid_path" || {
    printf 'canonical native path was rejected: %q\n' "$valid_path" >&2
    exit 1
  }
done
invalid_paths=(
  '' '/absolute' 'trailing/' 'empty//component' '.' '..' 'dir/.' 'dir/..'
  ' leading' 'trailing ' 'dir/ leading' 'dir/trailing '
  $'tab\tpath' $'newline\npath' $'control\001path'
  'backslash\path' 'glob*path' 'question?path' 'class[path'
  'dollar$path' 'semicolon;path' 'pipe|path' 'ampersand&path'
  'less<path' 'greater>path' 'paren(path'
)
for invalid_path in "${invalid_paths[@]}"; do
  if jain_native_relative_path_is_canonical "$invalid_path"; then
    printf 'non-canonical native path was accepted: %q\n' \
      "$invalid_path" >&2
    exit 1
  fi
done

validate_fixture_under_pipefail "$fixture_authority" "$fixture_root"
sort_guard="$tmp/sort-guard"
mkdir -p "$sort_guard"
printf '%s\n' '#!/usr/bin/env bash' \
  '[[ "${LC_ALL-}" == C ]] || exit 97' \
  'exec /usr/bin/sort "$@"' >"$sort_guard/sort"
chmod 0555 "$sort_guard/sort"
env -u LC_ALL LANG=en_US.UTF-8 PATH="$sort_guard:$PATH" \
  bash -c 'set -euo pipefail
source "$1"
jain_validate_native_build_tools "$2" "$3" content' -- \
    "$repo_root/ops/ci/native-runtime.sh" "$fixture_authority" "$fixture_root"
private_result_marker="$tmp/private-result-directory"
(
  mktemp() {
    local directory
    if [[ "$#" == 2 && "$1" == -d \
      && "$2" == /tmp/jain-native-inventory-result.XXXXXX ]]; then
      directory="$(command mktemp "$@")" || return 1
      printf '%s\n' "$directory" >"$private_result_marker"
      printf '%s\n' "$directory"
      return
    fi
    command mktemp "$@"
  }
  jain_validate_native_build_tools \
    "$fixture_authority" "$fixture_root" content
)
private_result_directory="$(<"$private_result_marker")"
[[ "$private_result_directory" \
    == /tmp/jain-native-inventory-result.?????? \
  && ! -e "$private_result_directory" ]] || {
  printf 'native validator did not use and remove a private result directory\n' \
    >&2
  exit 1
}
if jain_validate_native_build_tools \
  "$fixture_authority" "$fixture_root" root 2>/dev/null; then
  printf 'native build-tool validator accepted non-root fixture ownership\n' >&2
  exit 1
fi
ambient_bin="$tmp/ambient-bin"
mkdir -p "$ambient_bin"
printf '%s\n' '#!/usr/bin/env bash' 'exit 97' >"$ambient_bin/ld.lld"
chmod 0555 "$ambient_bin" "$ambient_bin/ld.lld"
prior_path="$ambient_bin:$PATH"
PATH="$prior_path"
jain_activate_native_build_tools "$fixture_authority" "$fixture_root"
[[ "$CMAKE" == "$fixture_root/bin/cmake" \
  && "$NINJA" == "$fixture_root/bin/ninja" \
  && "$CMAKE_MAKE_PROGRAM" == "$fixture_root/bin/ninja" \
  && "$PATH" == "$fixture_root/bin:$prior_path" \
  && "$(command -v ld.lld)" == "$fixture_root/bin/ld.lld" \
  && "$(ld.lld --version)" \
    == 'Ubuntu LLD 21.1.8 (compatible with GNU linkers)' ]] || exit 1
printf 'int main(void) { return 0; }\n' >"$tmp/lld-probe.c"
/usr/bin/clang -fuse-ld=lld "$tmp/lld-probe.c" -o "$tmp/lld-probe"
"$tmp/lld-probe"
PATH="${prior_path#"$ambient_bin:"}"

chmod 0755 "$fixture_root/bin"
mv -- "$fixture_root/bin/ld.lld" "$tmp/ld.lld.saved"
chmod 0555 "$fixture_root/bin"
if validate_fixture_under_pipefail \
  "$fixture_authority" "$fixture_root" 2>/dev/null; then
  printf 'native build-tool validator accepted a missing LLD\n' >&2
  exit 1
fi
chmod 0755 "$fixture_root/bin"
mv -- "$tmp/ld.lld.saved" "$fixture_root/bin/ld.lld"
chmod 0555 "$fixture_root/bin"

chmod 0644 "$fixture_root/bin/ninja"
if validate_fixture_under_pipefail \
  "$fixture_authority" "$fixture_root" 2>/dev/null; then
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
jq '.tools.lld.version = "Ubuntu LLD 0.0.0"' "$fixture_authority" \
  >"$tmp/wrong-version.json"
if jain_validate_native_build_tools \
  "$tmp/wrong-version.json" "$fixture_root" content 2>/dev/null; then
  printf 'native build-tool validator accepted a wrong version\n' >&2
  exit 1
fi
jq '.tools.lld.path = "bin/ninja"' "$fixture_authority" \
  >"$tmp/wrong-lld-path.json"
if jain_validate_native_build_tools \
  "$tmp/wrong-lld-path.json" "$fixture_root" content 2>/dev/null; then
  printf 'native build-tool validator accepted a substituted LLD path\n' >&2
  exit 1
fi
jq '.unreviewed = true' "$fixture_authority" >"$tmp/unknown-key.json"
if jain_validate_native_build_tools \
  "$tmp/unknown-key.json" "$fixture_root" content 2>/dev/null; then
  printf 'native build-tool validator accepted an unknown authority field\n' >&2
  exit 1
fi

large_stage="$tmp/native-build-tools-large-stage"
mkdir -p "$large_stage/bin" \
  "$large_stage/share/cmake-4.3/Help/generator" \
  "$large_stage/share/cmake-4.3/Modules" "$large_stage/zzzz"
for tool in cmake lld ninja ragel yasm; do
  relative="$tool"
  case "$tool" in
    cmake) version='cmake version 4.3.2' ;;
    lld)
      relative=ld.lld
      version='Ubuntu LLD 21.1.8 (compatible with GNU linkers)'
      ;;
    ninja) version='1.13.0.git.kitware.jobserver-pipe-1' ;;
    ragel) version='Ragel State Machine Compiler version 6.10 March 2017' ;;
    yasm) version='yasm 1.3.0' ;;
  esac
  printf '#!/usr/bin/env bash\nprintf "%%s\\n" "%s"\n' "$version" \
    >"$large_stage/bin/$relative"
done
printf 'space-bearing fixture\n' \
  >"$large_stage/share/cmake-4.3/Help/generator/Borland Makefiles.rst"
for index in $(seq -w 0 4017); do
  : >"$large_stage/share/cmake-4.3/Modules/InventoryFixture-$index.cmake"
done
printf 'late fixture\n' >"$large_stage/zzzz/LastFixture.txt"
find "$large_stage" -type d -exec chmod 0555 {} +
find "$large_stage" -type f -exec chmod 0444 {} +
chmod 0555 "$large_stage"/bin/*
large_authority="$tmp/native-build-tools-large.lock.json"
"$repo_root/ops/ci/native-build-tools-authority.sh" "$large_stage" \
  >"$large_authority"
large_inventory_sha="$(jq -er '.inventory_sha256' "$large_authority")"
large_root="$tmp/$large_inventory_sha"
mv -- "$large_stage" "$large_root"
large_inventory="$tmp/native-build-tools-large.inventory.tsv"
jain_write_native_build_tools_inventory \
  "$large_root" "$large_inventory" content
[[ "$(wc -l <"$large_inventory")" == 4025 \
  && "$(jq -er '.file_count' "$large_authority")" == 4025 \
  && "$(sha256sum -- "$large_inventory" | cut -d' ' -f1)" \
    == "$large_inventory_sha" \
  && "$(awk -F '\t' 'index($1, " ") { print NR ":" $1; exit }' \
      "$large_inventory")" \
    == '6:share/cmake-4.3/Help/generator/Borland Makefiles.rst' \
  && "$(tail -n 1 "$large_inventory" | cut -f1)" \
    == 'zzzz/LastFixture.txt' ]] || {
  printf 'large native inventory ordering or identity drifted\n' >&2
  exit 1
}
validate_fixture_under_pipefail "$large_authority" "$large_root"
chmod 0644 "$large_root/zzzz/LastFixture.txt"
printf 'late tamper\n' >"$large_root/zzzz/LastFixture.txt"
chmod 0444 "$large_root/zzzz/LastFixture.txt"
if validate_fixture_under_pipefail \
  "$large_authority" "$large_root" 2>/dev/null; then
  printf 'native validator missed a late post-space inventory tamper\n' >&2
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

cuda_output="$tmp/nvidia-smi.csv"
cuda_inventory="$tmp/nvidia-inventory.json"
printf '2, GPU-22222222-2222-2222-2222-222222222222, 8.6\n' \
  >"$cuda_output"
jain_parse_nvidia_compute_capability "$cuda_output" "$cuda_inventory"
jq -e 'length == 1 and .[0].index == 2
  and .[0].compute_cap == "8.6"
  and .[0].normalized_compute_cap == "86"' "$cuda_inventory" >/dev/null

rm -- "$cuda_output" "$cuda_inventory"
printf '7,GPU-77777777-7777-7777-7777-777777777777,8.6\n1,GPU-11111111-1111-1111-1111-111111111111,8.6\n' \
  >"$cuda_output"
jain_parse_nvidia_compute_capability "$cuda_output" "$cuda_inventory"
jq -e 'map(.index) == [1,7]
  and (map(.normalized_compute_cap) | unique) == ["86"]' \
  "$cuda_inventory" >/dev/null

request_id="$(printf 'a%.0s' {1..64})"
control_commit="$(printf 'b%.0s' {1..40})"
head_sha="$(printf 'c%.0s' {1..40})"
detector_sha="$(printf 'd%.0s' {1..64})"
cuda_record="$tmp/cuda-capability.json"
jain_write_cuda_capability_record "$cuda_inventory" "$cuda_record" \
  "$request_id" "$control_commit" veox jain-core "$head_sha" \
  jain-core/required /usr/bin/nvidia-smi "$detector_sha"
chmod 0444 "$cuda_record"
cuda_record_sha="$(sha256sum "$cuda_record" | cut -d' ' -f1)"
jain_verify_cuda_capability_record "$cuda_record" "$cuda_record_sha" \
  "$request_id" "$control_commit" veox jain-core "$head_sha" \
  jain-core/required "$detector_sha" 86 "$(id -u)" "$(id -g)"
if jain_verify_cuda_capability_record "$cuda_record" \
  "$(printf '0%.0s' {1..64})" "$request_id" "$control_commit" veox \
  jain-core "$head_sha" jain-core/required "$detector_sha" 86 \
  "$(id -u)" "$(id -g)"; then
  printf 'CUDA capability record accepted a mismatched digest\n' >&2
  exit 1
fi

invalid_cuda_outputs=(
  ''
  '0,GPU-00000000-0000-0000-0000-000000000000,N/A'
  '0,GPU-00000000-0000-0000-0000-000000000000,8'
  '0,GPU-00000000-0000-0000-0000-000000000000,8.60'
  '0,GPU-00000000-0000-0000-0000-000000000000,08.6'
  '0,GPU--------,8.6'
  '0,GPU-00000000-0000-0000-0000-000000000000,8.6,extra'
  $'0,GPU-00000000-0000-0000-0000-000000000000,8.6\n0,GPU-11111111-1111-1111-1111-111111111111,8.6'
  $'0,GPU-00000000-0000-0000-0000-000000000000,8.6\n1,GPU-00000000-0000-0000-0000-000000000000,8.6'
  $'0,GPU-00000000-0000-0000-0000-000000000000,8.6\n1,GPU-11111111-1111-1111-1111-111111111111,9.0'
)
for invalid_cuda_output in "${invalid_cuda_outputs[@]}"; do
  rm -f -- "$cuda_output" "$cuda_inventory"
  printf '%s' "$invalid_cuda_output" >"$cuda_output"
  if jain_parse_nvidia_compute_capability \
    "$cuda_output" "$cuda_inventory" 2>/dev/null; then
    printf 'CUDA parser accepted hostile output: %q\n' "$invalid_cuda_output" >&2
    exit 1
  fi
done
rm -f -- "$cuda_output" "$cuda_inventory"
head -c 65537 /dev/zero | tr '\0' x >"$cuda_output"
if jain_parse_nvidia_compute_capability \
  "$cuda_output" "$cuda_inventory" 2>/dev/null; then
  printf 'CUDA parser accepted oversized detector output\n' >&2
  exit 1
fi

detector_ok="$tmp/detector-ok"
detector_fail="$tmp/detector-fail"
detector_hang="$tmp/detector-hang"
detector_oversize="$tmp/detector-oversize"
cat >"$detector_ok" <<'SCRIPT'
#!/usr/bin/env bash
printf '0,GPU-00000000-0000-0000-0000-000000000000,8.6\n'
SCRIPT
cat >"$detector_fail" <<'SCRIPT'
#!/usr/bin/env bash
exit 9
SCRIPT
cat >"$detector_hang" <<'SCRIPT'
#!/usr/bin/env bash
sleep 5
SCRIPT
cat >"$detector_oversize" <<'SCRIPT'
#!/usr/bin/env bash
head -c 70000 /dev/zero | tr '\0' x
SCRIPT
chmod 0700 "$detector_ok" "$detector_fail" "$detector_hang" \
  "$detector_oversize"
rm -f -- "$cuda_output"
jain_run_nvidia_smi_detector "$detector_ok" "$cuda_output" 1
for bad_detector in /nonexistent/nvidia-smi "$detector_fail" \
  "$detector_hang" "$detector_oversize"; do
  rm -f -- "$cuda_output"
  if jain_run_nvidia_smi_detector "$bad_detector" "$cuda_output" 1; then
    printf 'bounded CUDA detector accepted failure: %s\n' "$bad_detector" >&2
    exit 1
  fi
done

printf 'native runtime build/link contract ok\n'
