#!/usr/bin/env bash
# Contract-drift lane: guards the machine-readable contract surface.
#
# The audit-policy pair (`.jankurai/audit-policy.toml` and its
# `agent/audit-policy.toml` twin) is the contract between the two tooling
# layers that read it; `scripts/check_audit_policy_mirror.sh` enforces
# byte equality. Any documents under `contracts/` must stay parseable
# JSON/JSONL, matching the jain-family contract-drift lanes. The sealed
# release CI dispatches this via `scripts/ci-local.sh contract-drift`.
#
# Usage:
#   bash ops/ci/contract-drift.sh

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

bash scripts/check_audit_policy_mirror.sh

if [ ! -d contracts ]; then
    printf 'contract drift ok: no contracts directory\n'
    exit 0
fi

python3 - <<'PY'
import json
from pathlib import Path

checked = 0
for path in sorted(Path("contracts").rglob("*")):
    if not path.is_file():
        continue
    if path.suffix == ".json":
        json.loads(path.read_text())
        checked += 1
    elif path.suffix == ".jsonl":
        for line in path.read_text().splitlines():
            if line.strip():
                json.loads(line)
        checked += 1
print(f"contract documents parsed: {checked}")
PY

printf 'contract drift ok\n'
