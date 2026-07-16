#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

for command in gitleaks cargo syft zizmor; do
  command -v "$command" >/dev/null 2>&1 || {
    printf 'missing required security command: %s\n' "$command" >&2
    exit 1
  }
done
cargo audit --version >/dev/null 2>&1 || {
  printf 'cargo-audit is required\n' >&2
  exit 1
}

mkdir -p target/jankurai/security
gitleaks detect --source . --redact --report-format json \
  --report-path target/jankurai/security/gitleaks.json
cargo audit --no-fetch --json > target/jankurai/security/cargo-audit.json
syft . -o spdx-json=target/jankurai/security/sbom.spdx.json
zizmor --offline --format json .github/workflows \
  > target/jankurai/security/zizmor.json
printf '{"schema_version":"redline-central.security/v1","status":"pass"}\n' \
  > target/jankurai/security/evidence.json
