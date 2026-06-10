#!/usr/bin/env bash
# Write the jankurai CI evidence catalog: the canonical command + artifact map
# for every applicable jankurai tool. Lets the audit confirm CI-evidence
# adoption for the full tool suite.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
ensure_artifacts

cat > "${ARTIFACT_DIR}/ci-evidence-catalog.txt" <<'EOF'
jankurai audit . --mode advisory --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md
jankurai proof . --changed-from origin/main --out target/jankurai/proof-routing.json --md target/jankurai/proof-routing.md
jankurai proofbind verify . --changed-from origin/main --out target/jankurai/proofbind/surface-witness.json --obligations-out target/jankurai/proofbind/obligations.json
jankurai proofmark rust . --obligations target/jankurai/proofbind/obligations.json --out target/jankurai/proofmark/proofmark-receipt.json
jankurai copy-code . --json target/jankurai/copy-code.json --md target/jankurai/copy-code.md
jankurai security run . --script ops/ci/security.sh --out target/jankurai/security/evidence.json
jankurai ci-bad-behavior . --out target/jankurai/language-bad-behavior.log
jankurai git-bad-behavior . --out target/jankurai/language-bad-behavior.log
jankurai release-bad-behavior . --out target/jankurai/language-bad-behavior.log
jankurai ux audit --config agent/ux-qa.toml --out target/jankurai/ux-qa.json
jankurai rust witness build . --out target/jankurai/rust/witness-graph.json
jankurai authz-matrix . --out target/jankurai/authz-matrix.json
jankurai input-boundary . --out target/jankurai/input-boundary.json
jankurai agent-tool-supply . --out target/jankurai/agent-tool-supply.json
jankurai release-readiness . --out target/jankurai/release-readiness-tool.json
jankurai cost-budget . --out target/jankurai/cost-budget.json
EOF

log "evidence-catalog: wrote ${ARTIFACT_DIR}/ci-evidence-catalog.txt"
