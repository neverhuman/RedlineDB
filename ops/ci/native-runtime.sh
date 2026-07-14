#!/usr/bin/env bash
# Shared runtime-library contract for release Cargo commands that enable the
# split family's native learners. This file is sourced by split-host-ci.sh.

jain_native_runtime_dirs() {
  local vendor_root="${1:?native vendor root is required}"
  printf '%s\n' \
    "$vendor_root/.build/catboost-nopy/catboost/libs/train_interface" \
    "$vendor_root/.build/xgboost-build" \
    "$vendor_root/.build/lightgbm-build"
}

jain_export_native_runtime_path() {
  local vendor_root="${1:?native vendor root is required}"
  local dir
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
  for dir in "${runtime_dirs[@]}"; do
    [[ -d "$dir" ]] || {
      printf 'native runtime directory does not exist: %s\n' "$dir" >&2
      return 1
    }
  done

  local joined
  joined="$(IFS=:; printf '%s' "${runtime_dirs[*]}")"
  if [[ -n "${LD_LIBRARY_PATH:-}" ]]; then
    joined="$joined:$LD_LIBRARY_PATH"
  fi
  export LD_LIBRARY_PATH="$joined"
}
