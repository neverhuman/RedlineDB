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
rm -f -- target/jankurai/security/evidence.json target/jankurai/security/evidence.json.tmp
trap 'rm -f -- target/jankurai/security/evidence.json.tmp' EXIT
gitleaks detect --source . --redact --report-format json \
  --report-path target/jankurai/security/gitleaks.json
bash ops/ci/pinned-rustsec-test.sh
# shellcheck source=ops/ci/pinned-rustsec.sh
source ops/ci/pinned-rustsec.sh
jain_resolve_governed_rustsec
cargo audit --db "$JAIN_RESOLVED_ADVISORY_DB" --no-fetch --json \
  > target/jankurai/security/cargo-audit.json
syft . -o spdx-json=target/jankurai/security/sbom.spdx.json
zizmor --offline --format json .github/workflows \
  > target/jankurai/security/zizmor.json
jq -n \
  --arg authority "$JAIN_RESOLVED_ADVISORY_AUTHORITY" \
  --arg commit "$JAIN_RESOLVED_ADVISORY_COMMIT" \
  --arg tree "$JAIN_RESOLVED_ADVISORY_TREE" \
  --arg audit_sha256 "$(sha256sum target/jankurai/security/cargo-audit.json | awk '{print $1}')" \
  --arg sbom_sha256 "$(sha256sum target/jankurai/security/sbom.spdx.json | awk '{print $1}')" \
  --arg gitleaks_sha256 "$(sha256sum target/jankurai/security/gitleaks.json | awk '{print $1}')" \
  --arg zizmor_sha256 "$(sha256sum target/jankurai/security/zizmor.json | awk '{print $1}')" \
  '{schema_version:"redline-central.security/v1",status:"pass",
    advisory:{authority:$authority,commit:$commit,tree:$tree},
    cargo_audit_sha256:$audit_sha256,sbom_sha256:$sbom_sha256,
    gitleaks_sha256:$gitleaks_sha256,zizmor_sha256:$zizmor_sha256}' \
  > target/jankurai/security/evidence.json.tmp
mv -- target/jankurai/security/evidence.json.tmp target/jankurai/security/evidence.json
trap - EXIT
