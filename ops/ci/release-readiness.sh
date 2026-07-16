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
./redlinectl control-review-lock-verify
./redlinectl release-receipt target/release-evidence/redline-release-readiness.json
