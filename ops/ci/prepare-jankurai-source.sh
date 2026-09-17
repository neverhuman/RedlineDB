#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=ops/ci/lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
cd "$repo_root"
source_dir="target/jankurai-source-$JANKURAI_VERSION"
if [[ ! -d "$source_dir/.git" ]]; then
  [[ ! -e "$source_dir" ]] || {
    printf 'Jankurai source path exists but is not a Git checkout: %s\n' "$source_dir" >&2
    exit 1
  }
  git clone --depth 1 --branch "v$JANKURAI_VERSION" "$JANKURAI_GIT" "$source_dir"
fi
[[ "$(git -C "$source_dir" rev-parse HEAD)" == "$JANKURAI_REV" ]] || {
  printf 'Jankurai source revision mismatch\n' >&2
  exit 1
}
