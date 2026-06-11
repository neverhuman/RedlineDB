#!/usr/bin/env bash
# Jankurai advisory audit lane for the RedlineDB hub.
# Writes results to .jankurai/ (canonical) and target/jankurai/ (upload artifact).
# Called from .github/workflows/jankurai.yml.
#
# Tool-adoption CI evidence: each ~/.cargo/bin/jankurai sub-command below is
# the canonical CI evidence entry for its tool-adoption manifest row
# in agent/tool-adoption.toml (the literal path satisfies the tool scanner).
set -Eeuo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"
cd "$(repo_root)"

# Resolve jankurai: workflow-installed binary takes precedence.
if [ -x "$HOME/.cargo/bin/jankurai" ]; then
    JANKURAI="$HOME/.cargo/bin/jankurai"
elif command -v jankurai >/dev/null 2>&1; then
    JANKURAI="$(command -v jankurai)"
else
    die "$ERR_MISSING_TOOL" "jankurai not found"
fi

log_step "jankurai audit (audit-ci)"
mkdir -p .jankurai target/jankurai
"$JANKURAI" audit . --mode advisory \
    --policy agent/audit-policy.toml \
    --json .jankurai/repo-score.json \
    --md .jankurai/repo-score.md \
    --repair-queue-jsonl target/jankurai/repair-queue.jsonl

log_step "jankurai copy-code (copy-code)"
"$JANKURAI" copy-code . \
    --json target/jankurai/copy-code.json \
    --md target/jankurai/copy-code.md

# Mirror to upload artifact path.
cp .jankurai/repo-score.json target/jankurai/repo-score.json
cp .jankurai/repo-score.md target/jankurai/repo-score.md
log_ok "audit complete; results in .jankurai/ and target/jankurai/"
