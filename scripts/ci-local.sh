#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
case "${1:-validate}" in
  validate) exec "$repo_root/redlinectl" validate ;;
  fast)
    cargo check --locked --manifest-path "$repo_root/Cargo.toml"
    exec cargo test --locked --manifest-path "$repo_root/Cargo.toml"
    ;;
  required)
    family_container="$repo_root/../redline-split"
    if [[ ! -d "$family_container" ]]; then
      family_source="${REDLINE_SPLIT_CONTAINER:-}"
      if [[ -z "$family_source" || ! -d "$family_source" ]]; then
        printf '%s\n' 'detached required CI needs REDLINE_SPLIT_CONTAINER pointing to the reviewed family checkout' >&2
        exit 2
      fi
      ln -s -- "$family_source" "$family_container"
    fi
    unset GIT_CONFIG_GLOBAL
    exec bash "$repo_root/ops/ci/quality-gates.sh"
    ;;
  family-ci) exec "$repo_root/redlinectl" family-ci ;;
  test) exec cargo test --locked --manifest-path "$repo_root/Cargo.toml" ;;
  security) exec bash "$repo_root/scripts/security.sh" ;;
  score) exec bash "$repo_root/scripts/score.sh" ;;
  release-readiness) exec bash "$repo_root/ops/ci/release-readiness.sh" ;;
  doctor) exec "$repo_root/redlinectl" doctor ;;
  *) printf 'usage: %s {validate|fast|required|family-ci|test|security|score|release-readiness|doctor}\n' "$0" >&2; exit 64 ;;
esac
