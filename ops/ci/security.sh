#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
cd "$repo_root"
expected_commands="cargo audit; cargo deny; gitleaks; actionlint; zizmor; syft SBOM"
[[ "${REQUIRED_SECURITY_COMMANDS:-$expected_commands}" == "$expected_commands" ]] || {
  printf 'security command contract differs from the governed lane\n' >&2
  exit 1
}
for tool in cargo-audit cargo-deny gitleaks actionlint zizmor syft; do
  require_tool "$tool"
done
mkdir -p target/security
cargo audit --deny warnings
cargo deny --all-features check
gitleaks detect --source . --config .gitleaks.toml --no-banner --redact --no-git
actionlint .github/workflows/*.yml
zizmor --min-severity high .github/workflows
syft dir:. -o spdx-json=target/security/redline-split-ops.spdx.json
./redlinectl security-receipt target/security/evidence.json
