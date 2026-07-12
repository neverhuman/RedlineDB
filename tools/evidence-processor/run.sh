#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
manifest="$repo_root/tools/evidence-processor/Cargo.toml"
target_dir="${REDLINE_CONTROL_TARGET_DIR:-$repo_root/target/evidence-processor}"

exec cargo run --locked --quiet \
  --manifest-path "$manifest" \
  --target-dir "$target_dir" \
  -- "$@"
