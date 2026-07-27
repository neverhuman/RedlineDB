#!/usr/bin/env bash
set -Eeuo pipefail

# tools/security-lane.sh is the canonical security wrapper for jankurai.
# It delegates to the maintained lane that runs gitleaks detect, cargo audit,
# cargo deny check, zizmor workflow linting, complete npm-lock Syft generation,
# and authenticated offline Grype. npm itself is lock/install integrity only.
# These are operational security commands, not echo-only proof. See
# agent/security-policy.toml for the tool policy.
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec "$ROOT_DIR/ops/ci/security.sh"
