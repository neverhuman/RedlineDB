#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=ops/ci/lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
cd "$repo_root"
expected_commands="cargo audit --no-fetch; cargo deny --frozen; gitleaks; actionlint; zizmor --offline; syft offline SBOM"
[[ "${REQUIRED_SECURITY_COMMANDS:-$expected_commands}" == "$expected_commands" ]] || {
  printf 'security command contract differs from the governed lane\n' >&2
  exit 1
}
for tool in cargo-audit cargo-deny gitleaks actionlint zizmor syft; do
  require_tool "$tool"
done
exec just --justfile "$repo_root/Justfile" security
