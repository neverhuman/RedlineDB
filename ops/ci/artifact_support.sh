#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

log 'artifact-support lane: record score and policy artifacts'
mkdir -p target/artifact-support
python3 - <<'PY'
from pathlib import Path
import json

artifacts = [
    "repos.manifest.toml",
    "agent/audit-policy.toml",
    ".jankurai/repo-score.json",
    ".jankurai/repo-score.md",
]
present = [path for path in artifacts if Path(path).exists()]
Path("target/artifact-support/receipt.json").write_text(
    json.dumps({"schema": "jain-split-ops.artifact-support/v1", "present": present}, indent=2) + "\n"
)
PY
printf 'artifact-support ok: jain-split-ops\n'

