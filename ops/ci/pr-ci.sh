#!/usr/bin/env bash
# redline-core PR-CI gate — the authoritative local jeryu check.
# Delegates to the canonical fast lane (preflight + the full test shard set),
# then runs an advisory jankurai audit. Independent of the other redline repos:
# green here means redline-core is green.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

# Canonical pre-merge gate (preflight checks + full test shards).
bash ops/ci/fast.sh

# Advisory jankurai audit (pinned 1.6.x binary; never the stale ~/.local shadow).
mkdir -p target/jankurai
JANKURAI="${JANKURAI_BIN:-$HOME/.cargo/bin/jankurai}"
[ -x "$JANKURAI" ] || JANKURAI="$(command -v jankurai || true)"
if [ -n "${JANKURAI:-}" ] && [ -x "$JANKURAI" ]; then
  "$JANKURAI" audit . --mode advisory \
    --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md || true
  echo "jankurai score written to target/jankurai/repo-score.md"
fi

echo "==> redline-core PR-CI: OK"
