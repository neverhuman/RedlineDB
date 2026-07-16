#!/usr/bin/env bash
# Write the jankurai CI evidence catalog: the canonical command + artifact map
# for every applicable jankurai tool. Lets the audit confirm CI-evidence
# adoption for the full tool suite.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
ensure_artifacts

cat > "${ARTIFACT_DIR}/ci-evidence-catalog.txt" <<'EOF'
bash ops/ci/governed-jankurai.sh audit . --mode advisory --full --no-score-history --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md
bash ops/ci/governed-jankurai.sh proof . --changed-from origin/main --out target/jankurai/proof-routing.json --md target/jankurai/proof-routing.md
bash ops/ci/governed-jankurai.sh proofbind verify . --changed-from origin/main --out target/jankurai/proofbind/surface-witness.json --obligations-out target/jankurai/proofbind/obligations.json
bash ops/ci/governed-jankurai.sh proofmark rust . --obligations target/jankurai/proofbind/obligations.json --out target/jankurai/proofmark/proofmark-receipt.json
bash ops/ci/governed-jankurai.sh copy-code . --json target/jankurai/copy-code.json --md target/jankurai/copy-code.md
bash ops/ci/governed-jankurai.sh security run . --script ops/ci/security.sh --out target/jankurai/security/evidence.json
bash ops/ci/governed-jankurai.sh ci-bad-behavior . --out target/jankurai/language-bad-behavior.log
bash ops/ci/governed-jankurai.sh git-bad-behavior . --out target/jankurai/language-bad-behavior.log
bash ops/ci/governed-jankurai.sh release-bad-behavior . --out target/jankurai/language-bad-behavior.log
bash ops/ci/governed-jankurai.sh ux audit --config agent/ux-qa.toml --out target/jankurai/ux-qa.json
bash ops/ci/governed-jankurai.sh rust witness build . --out target/jankurai/rust/witness-graph.json
bash ops/ci/governed-jankurai.sh authz-matrix . --out target/jankurai/authz-matrix.json
bash ops/ci/governed-jankurai.sh input-boundary . --out target/jankurai/input-boundary.json
bash ops/ci/governed-jankurai.sh agent-tool-supply . --out target/jankurai/agent-tool-supply.json
bash ops/ci/governed-jankurai.sh release-readiness . --out target/jankurai/release-readiness-tool.json
bash ops/ci/governed-jankurai.sh cost-budget . --out target/jankurai/cost-budget.json
EOF

log "evidence-catalog: wrote ${ARTIFACT_DIR}/ci-evidence-catalog.txt"
