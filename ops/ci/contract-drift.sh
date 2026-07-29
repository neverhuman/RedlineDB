#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

fail() {
    printf '[redline-contract-drift][error] %s\n' "$*" >&2
    exit 1
}

bash ops/ci/score.sh

for manifest in .jankurai/tool-adoption.toml agent/tool-adoption.toml; do
    declarations="$(grep -Ec '^id = "contract-drift"$' "$manifest")"
    [ "$declarations" -eq 1 ] \
        || fail "$manifest must declare contract-drift exactly once"
done

jq -e '
    [.. | objects | select(.id? == "contract-drift")] as $tools
    | ($tools | length) == 1
      and $tools[0].applicable == true
      and $tools[0].status == "artifact_verified"
      and ($tools[0].missing | type == "array" and length == 0)
      and $tools[0].ci_command
        == "jankurai audit . --mode ratchet --baseline target/jankurai/accepted-baseline.json --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md"
' target/jankurai/repo-score.json >/dev/null

receipt_dir="target/jankurai/contract-drift"
mkdir -p "$receipt_dir"
report_sha="$(sha256sum target/jankurai/repo-score.json | awk '{print $1}')"
project_manifest_sha="$(sha256sum .jankurai/tool-adoption.toml | awk '{print $1}')"
compat_manifest_sha="$(sha256sum agent/tool-adoption.toml | awk '{print $1}')"
jq -n \
    --arg head_sha "$(git rev-parse HEAD)" \
    --arg report_sha256 "$report_sha" \
    --arg project_manifest_sha256 "$project_manifest_sha" \
    --arg compatibility_manifest_sha256 "$compat_manifest_sha" \
    '{
        schema_version: "redline.contract-drift/v1",
        status: "pass",
        contract_role: "thin-hub-tool-adoption",
        head_sha: $head_sha,
        report_sha256: $report_sha256,
        project_manifest_sha256: $project_manifest_sha256,
        compatibility_manifest_sha256: $compatibility_manifest_sha256
    }' >"$receipt_dir/receipt.json"

printf '[redline-contract-drift] pass thin-hub contract-drift evidence verified\n'
