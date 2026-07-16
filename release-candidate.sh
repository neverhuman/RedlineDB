#!/usr/bin/env bash
# One-command, resumable Jain 8.0.1 release-candidate validation and dry deployment.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export JAIN_RELEASE_VERSION=8.0.1
export ATOMICSOUL_PUSH=0

exec cargo run --locked --quiet --manifest-path "$root/Cargo.toml" -- \
  release-candidate \
  --manifest "$root/repos.manifest.toml" \
  --evidence-dir "$root/docs/release-evidence/8.0.1/orchestrator" \
  "$@"
