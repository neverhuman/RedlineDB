#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

printf '[security:jain-split-ops] required secret, dependency, license, SBOM, and vulnerability scans\n' >&2
mkdir -p target/jankurai/security target/security

for tool in actionlint cargo cargo-audit cargo-deny gitleaks grype syft tee zizmor; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'required security tool is unavailable: %s\n' "$tool" >&2
    exit 1
  }
done

actionlint > target/security/actionlint.log
zizmor --offline --pedantic --min-severity high --format json \
  .github/workflows > target/security/zizmor.json

gitleaks detect --source . --no-git --redact --exit-code 1 \
  --report-format json --report-path target/security/gitleaks.json

cargo audit --deny warnings 2>&1 | tee target/security/cargo-audit.log
cargo deny check --config deny.toml 2>&1 | tee target/security/cargo-deny.log

syft scan dir:. --exclude './target/**' \
  --output spdx-json=target/security/sbom.spdx.json

grype sbom:target/security/sbom.spdx.json \
  --output json --file target/security/grype.json --fail-on high
cargo run --locked --quiet -- security-evidence \
  --zizmor target/security/zizmor.json \
  --gitleaks target/security/gitleaks.json \
  --sbom target/security/sbom.spdx.json \
  --grype target/security/grype.json \
  --receipt target/jankurai/security/evidence.json
cp target/jankurai/security/evidence.json target/security/evidence.json
printf 'security ok: jain-split-ops\n'
