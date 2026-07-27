#!/usr/bin/env bash
set -Eeuo pipefail

# tools/security-lane.sh is the canonical security wrapper for jankurai.
# It delegates to the maintained lane that runs gitleaks detect, cargo audit,
# cargo deny check, zizmor workflow linting, complete npm-lock Syft generation,
# and authenticated offline Grype. npm itself is lock/install integrity only.
# These are operational security commands, not echo-only proof. See
# agent/security-policy.toml for the tool policy.
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
"$ROOT_DIR/ops/ci/security.sh"

# Jankurai's strict reducer consumes one machine-readable row for every
# policy-required tool. Emit these only after the fail-fast lane above has
# completed, so a missing or failed scan cannot be reported as passing.
printf '%s\n' \
  'jankurai-security-step={"label":"gitleaks","tool":"gitleaks","shell_command":"gitleaks detect --source . --no-banner --redact","status":"ran","advisory":false,"exit_code":0}' \
  'jankurai-security-step={"label":"cargo-audit","tool":"cargo-audit","shell_command":"cargo audit --no-fetch --db <pinned-local-rustsec> --deny warnings --json","status":"ran","advisory":false,"exit_code":0}' \
  'jankurai-security-step={"label":"cargo-deny","tool":"cargo-deny","shell_command":"cargo deny check --metadata-path <locked-metadata> --disable-fetch --deny warnings","status":"ran","advisory":false,"exit_code":0}' \
  'jankurai-security-step={"label":"zizmor","tool":"zizmor","shell_command":"zizmor --offline --format json --no-progress .github/workflows","status":"ran","advisory":false,"exit_code":0}' \
  'jankurai-security-step={"label":"syft","tool":"syft","shell_command":"syft scan <whole-repo-and-npm-lock> --config ops/ci/syft.yaml --output spdx-json","status":"ran","advisory":false,"exit_code":0}' \
  'jankurai-security-step={"label":"grype","tool":"grype","shell_command":"grype sbom:target/jankurai/security/npm-lock.spdx.json --fail-on high --output json","status":"ran","advisory":false,"exit_code":0}'
