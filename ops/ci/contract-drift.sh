#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

log 'contract-drift lane: validate manifest and local-Jeryu policy contracts'
python3 ops/split/validate-local-jeryu.py --manifest repos.manifest.toml
python3 - <<'PY'
from pathlib import Path
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

manifest = tomllib.loads(Path("repos.manifest.toml").read_text())
if len(manifest.get("repo", [])) != 19:
    raise SystemExit("manifest must describe 19 live family repos")
PY
write_receipt target/jankurai/contract-drift/receipt.json pass
printf 'contract-drift ok: jain-split-ops\n'

