#!/usr/bin/env bash
# Jankurai tool-suite evidence lane. Runs the applicable jankurai tools and
# writes their artifacts under target/jankurai/** so CI can upload one evidence
# bundle. The governed 1.6.11 identity is a hard prerequisite; supplementary
# analysis failures are recorded only after that verification.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
ensure_artifacts
mkdir -p "${ARTIFACT_DIR}/proofbind" "${ARTIFACT_DIR}/proofmark" \
  "${ARTIFACT_DIR}/security" "${ARTIFACT_DIR}/rust"

require_governed_jankurai

status=0
base_ref="${JANKURAI_BASE_REF:-origin/main}"

run_step() {
  local name="$1"; shift
  log "jankurai: $name"
  if ! "$@"; then
    warn "jankurai: $name failed"
    status=1
  fi
}

run_step_soft() {
  local name="$1"; shift
  log "jankurai: $name"
  "$@" || warn "jankurai: $name failed (non-fatal; supplementary evidence lane)"
}

run_step "audit-ci (score)" \
  bash ops/ci/run-jankurai.sh audit . --mode advisory \
    --json "${ARTIFACT_DIR}/repo-score.json" \
    --md "${ARTIFACT_DIR}/repo-score.md" \
    --repair-queue-jsonl "${ARTIFACT_DIR}/repair-queue.jsonl"

run_step_soft "proof-routing" \
  bash ops/ci/run-jankurai.sh proof . --changed-from "$base_ref" \
    --out "${ARTIFACT_DIR}/proof-routing.json" --md "${ARTIFACT_DIR}/proof-routing.md"

run_step_soft "proofbind" \
  bash ops/ci/run-jankurai.sh proofbind verify . --changed-from "$base_ref" \
    --out "${ARTIFACT_DIR}/proofbind/surface-witness.json" \
    --obligations-out "${ARTIFACT_DIR}/proofbind/obligations.json"

run_step_soft "proofmark-rust" \
  bash ops/ci/run-jankurai.sh proofmark rust . --obligations "${ARTIFACT_DIR}/proofbind/obligations.json" \
    --out "${ARTIFACT_DIR}/proofmark/proofmark-receipt.json"

run_step "copy-code" \
  bash ops/ci/run-jankurai.sh copy-code . --json "${ARTIFACT_DIR}/copy-code.json" --md "${ARTIFACT_DIR}/copy-code.md"

run_step "security evidence" \
  bash ops/ci/run-jankurai.sh security run . --script ops/ci/security.sh --out "${ARTIFACT_DIR}/security/evidence.json"

run_step "language bad-behavior" bash ops/ci/language-bad-behavior.sh

run_step_soft "rust-witness" \
  bash ops/ci/run-jankurai.sh rust witness build . --out "${ARTIFACT_DIR}/rust/witness-graph.json"

# authz-matrix / input-boundary / agent-tool-supply are audit detectors; the
# audit run below produces their evidence in the repo score JSON.
run_step_soft "authz-matrix + input-boundary + agent-tool-supply (audit detectors)" \
  bash ops/ci/run-jankurai.sh audit . --mode advisory \
    --json "${ARTIFACT_DIR}/authz-matrix.json" --md "${ARTIFACT_DIR}/authz-matrix.md"

run_step "contract-drift" bash ops/ci/contract-drift.sh
run_step "release-readiness (script)" bash ops/ci/release-readiness.sh
run_step "cost-budget" bash ops/ci/cost-budget.sh
run_step "ux-qa" bash ops/ci/ux-qa.sh

exit "$status"
