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
./redlinectl review-lock-verify
./redlinectl successor-receipt-verify \
  release-evidence/8.0.0/redline-proof-successor-jain4-prepared.json
reconciled=release-evidence/8.0.0/redline-proof-successor-jain4-reconciled.json
if [[ -e "$reconciled" || -e "$reconciled.sha256" ]]; then
  [[ -s "$reconciled" && -s "$reconciled.sha256" ]] || {
    printf 'release readiness: incomplete reconciled successor receipt\n' >&2
    exit 1
  }
  ./redlinectl successor-receipt-verify "$reconciled"
fi
./redlinectl release-receipt target/release-evidence/redline-release-readiness.json
