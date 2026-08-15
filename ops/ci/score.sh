#!/usr/bin/env bash
# Score lane: floor-gated jankurai audit for the release proof surface.
#
# Runs the same full advisory audit as `just score` against
# agent/audit-policy.toml, then enforces the policy floor
# (`minimum_score`), zero caps, and zero hard findings — the same gate
# shape as the jeryu-release-ops and jain-family score lanes. The sealed
# release CI dispatches this via `scripts/ci-local.sh score`.
#
# Usage:
#   bash ops/ci/score.sh

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

expected_jankurai="jankurai 1.6.11"
actual_jankurai="$(jankurai --version 2>/dev/null || true)"
if [ "$actual_jankurai" != "$expected_jankurai" ]; then
    printf 'score lane requires %s, found %s\n' \
        "$expected_jankurai" "${actual_jankurai:-no jankurai on PATH}" >&2
    exit 1
fi

required_metadata=(
    agent/audit-policy.toml
    agent/boundaries.toml
    agent/tool-adoption.toml
    agent/JANKURAI_STANDARD.md
    .jankurai/owner-map.json
    .jankurai/test-map.json
    .jankurai/generated-zones.toml
    .jankurai/proof-lanes.toml
)
for path in "${required_metadata[@]}"; do
    [ -s "$path" ] || { printf 'missing split metadata: %s\n' "$path" >&2; exit 1; }
done

bash scripts/check_audit_policy_mirror.sh

mkdir -p target/jankurai
# CI jobs start from an empty target directory; local checkouts may keep
# smart-scan state from earlier audits. Removing it forces a full
# evidence scan while keeping the command canonical (same rationale as
# ops/ci/jankurai-audit.sh).
rm -f target/jankurai/audit-state.json

jankurai audit . \
    --full \
    --mode advisory \
    --policy agent/audit-policy.toml \
    --json target/jankurai/repo-score.json \
    --md target/jankurai/repo-score.md \
    --no-score-history

python3 - <<'PY'
import json
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

report = json.loads(Path("target/jankurai/repo-score.json").read_text())
policy = tomllib.loads(Path("agent/audit-policy.toml").read_text())
score = int(report.get("score") or 0)
floor = int(policy.get("minimum_score", 85))
caps = report.get("caps_applied") or report.get("caps") or []
decision = report.get("decision") if isinstance(report.get("decision"), dict) else {}
hard = decision.get("hard_findings", report.get("hard_findings", 0))
hard_count = len(hard) if isinstance(hard, list) else int(hard or 0)
errors = []
if score < floor:
    errors.append(f"score {score} is below the policy floor {floor}")
if caps:
    errors.append("caps present: " + ", ".join(str(item) for item in caps))
if hard_count:
    errors.append(f"hard findings present: {hard_count}")
if errors:
    print("score check failed: " + "; ".join(errors), file=sys.stderr)
    sys.exit(1)
print(f"score ok: {score} (floor {floor}; no caps; no hard findings)")
PY
