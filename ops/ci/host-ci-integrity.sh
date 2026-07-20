#!/usr/bin/env bash
# Fail before sourcing or executing mutable host-CI orchestration bytes.
set -euo pipefail

if [[ "${JAIN_HOST_CI_INTEGRITY_CLEAN:-0}" != 1 ]]; then
  exec /usr/bin/env -i PATH=/usr/bin:/bin LC_ALL=C \
    JAIN_HOST_CI_INTEGRITY_CLEAN=1 /bin/bash "${BASH_SOURCE[0]}" "$@"
fi

ops_root="${1:?control-plane root is required}"
shift
expected_commit=""
authority_ref=""
case "${1:-}" in
  "") ;;
  --ref)
    authority_ref="${2:?control-plane authority ref is required}"
    [[ "$#" == 2 ]] || exit 1
    ;;
  *)
    expected_commit="$1"
    [[ "$#" == 1 ]] || exit 1
    ;;
esac
[[ "$ops_root" = /* ]] || {
  printf 'host CI control-plane root must be absolute: %s\n' "$ops_root" >&2
  exit 1
}
ops_root="$(realpath -e -- "$ops_root")"
safe_git=(git -c safe.directory="$ops_root" -c core.fsmonitor=false \
  -c core.hooksPath=/dev/null -c core.untrackedCache=false -c diff.external=)
if [[ -n "$authority_ref" ]]; then
  [[ "$authority_ref" =~ ^refs/remotes/origin/[a-zA-Z0-9][a-zA-Z0-9._/-]*[a-zA-Z0-9]$ \
    && "$authority_ref" != *..* && "$authority_ref" != *//* \
    && "$authority_ref" != *@\{* && "$authority_ref" != *.lock ]] || {
    printf 'host CI control-plane authority ref is unsafe: %s\n' \
      "$authority_ref" >&2
    exit 1
  }
  commit="$("${safe_git[@]}" -C "$ops_root" \
    rev-parse --verify "$authority_ref^{commit}" 2>/dev/null)" || {
    printf 'host CI cannot resolve the published control-plane authority ref: %s\n' \
      "$authority_ref" >&2
    exit 1
  }
  # Close a concurrent local ref update before returning the digest. Root will
  # independently authenticate the corresponding forge ref and exact commit.
  [[ "$("${safe_git[@]}" -C "$ops_root" \
    rev-parse --verify "$authority_ref^{commit}" 2>/dev/null)" == "$commit" ]] || {
    printf 'host CI control-plane authority ref moved while reading\n' >&2
    exit 1
  }
else
  commit="$("${safe_git[@]}" -C "$ops_root" \
    rev-parse --verify 'HEAD^{commit}' 2>/dev/null)" || {
  printf 'host CI cannot resolve the control-plane commit\n' >&2
  exit 1
  }
fi
if [[ -n "$expected_commit" && "$commit" != "$expected_commit" ]]; then
  printf 'host CI control-plane commit changed: %s != %s\n' \
    "$commit" "$expected_commit" >&2
  exit 1
fi

if [[ -z "$authority_ref" && -n "$("${safe_git[@]}" -C "$ops_root" \
  status --porcelain=v1 --untracked-files=all)" ]]; then
  printf 'host CI refuses a dirty control-plane worktree: %s\n' "$ops_root" >&2
  exit 1
fi

paths=(
  Cargo.lock
  Cargo.toml
  repos.manifest.toml
  ops/ci/host-ci-integrity.sh
  ops/ci/host-ci-publisher.sh
  ops/ci/host-ci-sandbox.sh
  ops/ci/host-ci-boundary-preflight.sh
  ops/ci/host-ci-proof-evidence.sh
  ops/ci/native-build-tools.lock.json
  ops/ci/native-runtime.sh
  ops/ci/pnpm-runtime.sh
  ops/ci/pnpm-store.lock.json
  ops/ci/pinned-advisory.sh
  ops/ci/pinned-cargo-audit.sh
  ops/ci/pinned-cargo-deny.sh
  ops/ci/split-host-ci-parent.sh
  ops/ci/split-host-ci.sh
  tools/splitctl/src/jeryu_client.rs
  tools/splitctl/src/main.rs
)
for path in "${paths[@]}"; do
  "${safe_git[@]}" -C "$ops_root" cat-file -e "$commit:$path" 2>/dev/null || {
    printf 'host CI commit does not bind orchestration input: %s\n' "$path" >&2
    exit 1
  }
  if [[ -z "$authority_ref" ]]; then
    [[ -f "$ops_root/$path" && ! -L "$ops_root/$path" ]] || {
      printf 'host CI orchestration input is missing or linked: %s\n' "$path" >&2
      exit 1
    }
    working_sha="$(sha256sum -- "$ops_root/$path" | cut -d' ' -f1)"
    committed_sha="$("${safe_git[@]}" -C "$ops_root" show "$commit:$path" \
      | sha256sum | cut -d' ' -f1)"
    [[ "$working_sha" == "$committed_sha" ]] || {
      printf 'host CI orchestration digest mismatch: %s\n' "$path" >&2
      exit 1
    }
  fi
done

printf '%s\n' "$commit"
