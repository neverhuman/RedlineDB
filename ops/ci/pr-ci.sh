#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"

cd "$repo_root"

# scripts/release-package.sh copies from target/release, so keep this lane's
# release build output inside the repo even when host-ci provides a shared cache.
export CARGO_TARGET_DIR="$repo_root/target"

# Pre-flight: keep `.jankurai/audit-policy.toml` and `agent/audit-policy.toml`
# in sync. The two are read by different tooling layers; drift silently
# changes the active audit policy from what's documented in .jankurai/.
ci_run scripts/check_audit_policy_mirror.sh

ci_run cargo fmt --check
ci_run cargo check --locked
ci_run cargo test --locked
ci_run cargo test --locked -p xtask
ci_run tests/release_lanes_hostile.sh
ci_run env REDLINE_TESTING_RELEASE_TAG="${REDLINE_TESTING_RELEASE_TAG:-redline-testing-v1.0.1-jain.2}" \
    scripts/release-package.sh

# Security evidence is part of the required lane. Missing scanners fail closed
# so a runner cannot report success from a partial tool installation.
ci_run env REDLINE_STRICT_TOOLS=1 bash "$repo_root/ops/ci/security.sh"
