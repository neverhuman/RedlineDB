#!/usr/bin/env bash
#
# Release-readiness lane: asserts the launch-gate evidence surface is present
# and emits a receipt. The harness ships a versioned tarball + manifest, so the
# gate proves security, rollback, monitoring, and provenance are documented.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"

cd "$repo_root"
mkdir -p target/jankurai

log "release-readiness: validating launch-gate evidence"
cargo run --locked --quiet -p xtask -- release-readiness

log "release-readiness: complete"
