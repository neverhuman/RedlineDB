#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

printf '[security:jain-split-ops] required secret, dependency, license, SBOM, and vulnerability scans\n' >&2
mkdir -p target/jankurai/security target/security

for tool in actionlint cargo cargo-audit cargo-deny gitleaks grype jq sha256sum syft tee zizmor; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'required security tool is unavailable: %s\n' "$tool" >&2
    exit 1
  }
done

actionlint > target/security/actionlint.log
zizmor --offline --pedantic --min-severity high --format json \
  .github/workflows > target/security/zizmor.json
jq -e 'type == "array" and length == 0' target/security/zizmor.json >/dev/null

gitleaks detect --source . --no-git --redact --exit-code 1 \
  --report-format json --report-path target/security/gitleaks.json
jq -e 'type == "array" and length == 0' target/security/gitleaks.json >/dev/null

cargo audit --deny warnings 2>&1 | tee target/security/cargo-audit.log
cargo deny check --config deny.toml 2>&1 | tee target/security/cargo-deny.log

syft scan dir:. --exclude './target/**' \
  --output spdx-json=target/security/sbom.spdx.json
jq -e '.spdxVersion and (.packages | type == "array")' \
  target/security/sbom.spdx.json >/dev/null

grype sbom:target/security/sbom.spdx.json \
  --output json --file target/security/grype.json --fail-on high
jq -e '(.matches | type == "array") and (.source | type == "object")' \
  target/security/grype.json >/dev/null

read -r sbom_sha _ < <(sha256sum target/security/sbom.spdx.json)
read -r grype_sha _ < <(sha256sum target/security/grype.json)
package_count="$(jq '.packages | length' target/security/sbom.spdx.json)"
high_or_critical="$(jq '[.matches[]? | select(.vulnerability.severity == "High" or .vulnerability.severity == "Critical")] | length' target/security/grype.json)"

jq -n \
  --arg sbom_sha256 "$sbom_sha" \
  --arg grype_sha256 "$grype_sha" \
  --argjson package_count "$package_count" \
  --argjson high_or_critical "$high_or_critical" \
  '{schema:"jain-split-ops.security/v1",status:"pass",scans:["actionlint","zizmor","gitleaks","cargo-audit","cargo-deny","syft","grype"],fallbacks:false,sbom:{format:"spdx-json",sha256:$sbom_sha256,packages:$package_count},vulnerabilities:{grype_sha256:$grype_sha256,fail_on:"high",high_or_critical:$high_or_critical}}' \
  > target/jankurai/security/evidence.json
cp target/jankurai/security/evidence.json target/security/evidence.json
printf 'security ok: jain-split-ops\n'
