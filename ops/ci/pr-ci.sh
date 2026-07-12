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
ci_run scripts/release-package.sh

# Security scanning (secret + dependency + workflow lint). Skips any tool that
# is not installed locally, so this stays green during bootstrap while still
# running every scan that IS available. The dedicated CI `security` job runs
# the same script with the tools provisioned.
ci_run bash "$repo_root/ops/ci/security.sh"
