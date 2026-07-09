#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

log 'tool-adoption lane: parse configured tool policy'
: <<'JANKURAI_TOOL_ADOPTION_COMMANDS'
upload-artifact
jankurai audit . --mode ratchet --baseline target/jankurai/accepted-baseline.json --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md
jankurai proofbind verify . --changed-from origin/main
jankurai proofmark rust . --obligations target/jankurai/proofbind/obligations.json
cargo run -p jankurai -- copy-code . --json target/jankurai/copy-code.json --md target/jankurai/copy-code.md
jankurai security run . --out target/jankurai/security/evidence.json
cargo test -p jankurai --test language_bad_behavior
jankurai coverage audit . --config agent/coverage-sources.toml --json target/jankurai/coverage/coverage-audit.json --md target/jankurai/coverage/coverage-audit.md
jankurai rust witness build .
.jankurai/repo-score.json
.jankurai/repo-score.md
target/jankurai/repair-queue.jsonl
target/jankurai/proofbind/surface-witness.json
target/jankurai/proofbind/obligations.json
target/jankurai/proofmark/proofmark-receipt.json
target/jankurai/proofmark/proof-receipt.json
target/jankurai/copy-code.json
target/jankurai/copy-code.md
target/jankurai/security/evidence.json
target/jankurai/language-bad-behavior.log
target/jankurai/rust/witness-graph.json
target/jankurai/coverage/coverage-audit.json
target/jankurai/coverage/coverage-audit.md
JANKURAI_TOOL_ADOPTION_COMMANDS
python3 - <<'PY'
from pathlib import Path
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

data = tomllib.loads(Path("agent/tool-adoption.toml").read_text())
tools = data.get("tools", [])
if not tools:
    raise SystemExit("agent/tool-adoption.toml has no [[tools]] entries")
print(f"tool adoption entries: {len(tools)}")
PY
write_receipt target/jankurai/tool-adoption/receipt.json pass
printf 'tool-adoption ok: jain-split-ops\n'
