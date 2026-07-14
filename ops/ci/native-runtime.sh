#!/usr/bin/env bash
# Shared runtime-library contract for release Cargo commands that enable the
# split family's native learners. This file is sourced by split-host-ci.sh.

jain_native_learners_for_repo() {
  local repo="${1:?repository name is required}"
  case "$repo" in
    jain-catboost) printf '%s\n' catboost ;;
    jain-xgboost) printf '%s\n' xgboost ;;
    jain-lightgbm) printf '%s\n' lightgbm ;;
    jain-core | jain-cli | jain-web | jain-deploy)
      printf '%s\n' catboost xgboost lightgbm
      ;;
  esac
}

jain_native_build_dirs() {
  local vendor_root="${1:?native vendor root is required}"
  printf '%s\n' \
    "$vendor_root/.build/catboost-nopy" \
    "$vendor_root/.build/xgboost-build" \
    "$vendor_root/.build/lightgbm-build"
}

jain_native_runtime_dirs() {
  local vendor_root="${1:?native vendor root is required}"
  printf '%s\n' \
    "$vendor_root/.build/catboost-nopy/catboost/libs/train_interface" \
    "$vendor_root/.build/xgboost-build" \
    "$vendor_root/.build/lightgbm-build"
}

jain_native_library_path() {
  local vendor_root="${1:?native vendor root is required}"
  local learner="${2:?native learner is required}"
  case "$learner" in
    catboost)
      printf '%s\n' \
        "$vendor_root/.build/catboost-nopy/catboost/libs/train_interface/libcatboost.so"
      ;;
    xgboost) printf '%s\n' "$vendor_root/.build/xgboost-build/libxgboost.so" ;;
    lightgbm) printf '%s\n' "$vendor_root/.build/lightgbm-build/lib_lightgbm.so" ;;
    *)
      printf 'unknown native learner: %s\n' "$learner" >&2
      return 1
      ;;
  esac
}

jain_prepare_native_runtime() {
  local vendor_root="${1:?native vendor root is required}"
  local learner dir
  local -a build_dirs=()

  case "$vendor_root" in
    /*) ;;
    *)
      printf 'native runtime root must be absolute: %s\n' "$vendor_root" >&2
      return 1
      ;;
  esac
  [[ -d "$vendor_root" ]] || {
    printf 'native runtime root does not exist: %s\n' "$vendor_root" >&2
    return 1
  }
  for learner in catboost xgboost lightgbm; do
    [[ -d "$vendor_root/$learner" ]] || {
      printf 'native source directory does not exist: %s\n' "$vendor_root/$learner" >&2
      return 1
    }
  done

  # These are build destinations, not evidence that a learner built. In
  # particular, never create CatBoost's nested final library directory here:
  # only the CatBoost build is allowed to produce it.
  mapfile -t build_dirs < <(jain_native_build_dirs "$vendor_root")
  mkdir -p "${build_dirs[@]}"
  for dir in "${build_dirs[@]}"; do
    [[ -d "$dir" ]] || {
      printf 'native build destination is unavailable: %s\n' "$dir" >&2
      return 1
    }
  done

  export JAIN_VENDOR_ROOT="$vendor_root"
  export CATBOOST_BUILD_DIR="${build_dirs[0]}"
  export XGBOOST_BUILD_DIR="${build_dirs[1]}"
  export LIGHTGBM_BUILD_DIR="${build_dirs[2]}"
  jain_export_native_runtime_path "$vendor_root"
}

jain_export_native_runtime_path() {
  local vendor_root="${1:?native vendor root is required}"
  local -a runtime_dirs=()

  case "$vendor_root" in
    /*) ;;
    *)
      printf 'native runtime root must be absolute: %s\n' "$vendor_root" >&2
      return 1
      ;;
  esac
  [[ -d "$vendor_root" ]] || {
    printf 'native runtime root does not exist: %s\n' "$vendor_root" >&2
    return 1
  }

  mapfile -t runtime_dirs < <(jain_native_runtime_dirs "$vendor_root")

  local joined
  joined="$(IFS=:; printf '%s' "${runtime_dirs[*]}")"
  if [[ -n "${LD_LIBRARY_PATH:-}" ]]; then
    joined="$joined:$LD_LIBRARY_PATH"
  fi
  export LD_LIBRARY_PATH="$joined"
}

jain_verify_elf_dependencies() {
  local artifact="${1:?ELF artifact is required}"
  local description output

  [[ -f "$artifact" && -s "$artifact" ]] || {
    printf 'native artifact is missing or empty: %s\n' "$artifact" >&2
    return 1
  }
  command -v file >/dev/null 2>&1 || {
    printf 'required native verification tool is unavailable: file\n' >&2
    return 1
  }
  command -v ldd >/dev/null 2>&1 || {
    printf 'required native verification tool is unavailable: ldd\n' >&2
    return 1
  }
  description="$(file -b -- "$artifact")" || return 1
  case "$description" in
    ELF*"dynamically linked"* | ELF*"shared object"*) ;;
    *)
      printf 'native artifact is not a dynamically linked ELF: %s (%s)\n' \
        "$artifact" "$description" >&2
      return 1
      ;;
  esac
  if ! output="$(ldd "$artifact" 2>&1)"; then
    printf 'ldd failed for native artifact %s:\n%s\n' "$artifact" "$output" >&2
    return 1
  fi
  if grep -Fq 'not found' <<<"$output"; then
    printf 'unresolved runtime dependency for %s:\n%s\n' "$artifact" "$output" >&2
    return 1
  fi
}

jain_verify_native_libraries() {
  local vendor_root="${1:?native vendor root is required}"
  shift
  local learner library
  local -a learners=("$@")
  if [[ "${#learners[@]}" -eq 0 ]]; then
    learners=(catboost xgboost lightgbm)
  fi
  for learner in "${learners[@]}"; do
    library="$(jain_native_library_path "$vendor_root" "$learner")" || return 1
    [[ -f "$library" && -s "$library" ]] || {
      printf '%s release build did not produce exact non-empty library: %s\n' \
        "$learner" "$library" >&2
      return 1
    }
    jain_verify_elf_dependencies "$library" || return 1
  done
}

jain_verify_linked_binaries() {
  local release_dir="${1:?Cargo release directory is required}"
  local artifact description output
  local checked=0

  [[ -d "$release_dir" ]] || {
    printf 'Cargo release directory does not exist: %s\n' "$release_dir" >&2
    return 1
  }
  while IFS= read -r -d '' artifact; do
    description="$(file -b -- "$artifact")" || return 1
    case "$description" in
      ELF*"dynamically linked"* | ELF*"shared object"*) ;;
      *) continue ;;
    esac
    if ! output="$(ldd "$artifact" 2>&1)"; then
      printf 'ldd failed for linked release artifact %s:\n%s\n' \
        "$artifact" "$output" >&2
      return 1
    fi
    if grep -Fq 'not found' <<<"$output"; then
      printf 'unresolved runtime dependency for %s:\n%s\n' \
        "$artifact" "$output" >&2
      return 1
    fi
    checked=$((checked + 1))
  done < <(find "$release_dir" -type f \
    \( -perm /111 -o -name '*.so' -o -name '*.so.*' \) -print0)
  [[ "$checked" -gt 0 ]] || {
    printf 'native release build produced no linked ELF artifacts under %s\n' \
      "$release_dir" >&2
    return 1
  }
}
