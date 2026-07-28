#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=ops/ci/lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
cd "$repo_root"
for path in Cargo.toml Cargo.lock CHANGELOG.md RELEASE_PROCESS.md ROLLBACK.md docs/release.md docs/testing.md; do
  [[ -s "$path" ]] || {
    printf 'release readiness: missing required evidence: %s\n' "$path" >&2
    exit 1
  }
done
./redlinectl control-validate
mirror="$repo_root/../redline.lock.toml"
mirror_sidecar="${mirror}.sha256"
if [[ ! -e "$mirror" && ! -L "$mirror" && ! -e "$mirror_sidecar" && ! -L "$mirror_sidecar" ]]; then
  ./redlinectl release-receipt --standalone target/release-evidence/redline-release-readiness.json
else
  ./redlinectl release-receipt target/release-evidence/redline-release-readiness.json
fi
