#!/usr/bin/env bash
# Fail before sourcing or executing mutable host-CI orchestration bytes.
set -euo pipefail

ops_root="${1:?control-plane root is required}"
[[ "$ops_root" = /* ]] || {
  printf 'host CI control-plane root must be absolute: %s\n' "$ops_root" >&2
  exit 1
}
ops_root="$(realpath -e -- "$ops_root")"
commit="$(git -C "$ops_root" rev-parse --verify 'HEAD^{commit}' 2>/dev/null)" || {
  printf 'host CI cannot resolve the control-plane commit\n' >&2
  exit 1
}

if [[ -n "$(git -C "$ops_root" status --porcelain=v1 --untracked-files=all)" ]]; then
  printf 'host CI refuses a dirty control-plane worktree: %s\n' "$ops_root" >&2
  exit 1
fi

paths=(
  Cargo.lock
  Cargo.toml
  repos.manifest.toml
  ops/ci/host-ci-integrity.sh
  ops/ci/native-runtime.sh
  ops/ci/pinned-advisory.sh
  ops/ci/pinned-cargo-audit.sh
  ops/ci/pinned-cargo-deny.sh
  ops/ci/split-host-ci.sh
  tools/splitctl/src/main.rs
)
for path in "${paths[@]}"; do
  [[ -f "$ops_root/$path" && ! -L "$ops_root/$path" ]] || {
    printf 'host CI orchestration input is missing or linked: %s\n' "$path" >&2
    exit 1
  }
  git -C "$ops_root" cat-file -e "$commit:$path" 2>/dev/null || {
    printf 'host CI commit does not bind orchestration input: %s\n' "$path" >&2
    exit 1
  }
  working_sha="$(sha256sum -- "$ops_root/$path" | cut -d' ' -f1)"
  committed_sha="$(git -C "$ops_root" show "$commit:$path" | sha256sum | cut -d' ' -f1)"
  [[ "$working_sha" == "$committed_sha" ]] || {
    printf 'host CI orchestration digest mismatch: %s\n' "$path" >&2
    exit 1
  }
done

printf '%s\n' "$commit"
