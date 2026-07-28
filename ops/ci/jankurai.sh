#!/usr/bin/env bash
# Jankurai tool-suite evidence lane. Every command is governed and blocking;
# all run evidence stays under ignored target/jankurai/**.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
ensure_artifacts
mkdir -p "${ARTIFACT_DIR}/proofbind" "${ARTIFACT_DIR}/proofmark" \
  "${ARTIFACT_DIR}/security" "${ARTIFACT_DIR}/rust"

JBIN="$(jankurai_bin)" || fail "governed Jankurai identity verification failed"
[[ -z "$(git status --porcelain)" ]] || fail "Jankurai proof requires a clean checkout at start"

status=0
if [[ "${JAIN_RELEASE_CI:-0}" == "1" ]]; then
  : "${JAIN_CONTRACT_BASE_REF:?release proof base commit is required}"
  base_ref="$JAIN_CONTRACT_BASE_REF"
  [[ "$base_ref" =~ ^[0-9a-f]{40}$ ]] \
    || fail "release proof base must be a full lowercase commit: ${base_ref}"
else
  base_ref="${JANKURAI_BASE_REF:-origin/main}"
fi
base_commit="$(git rev-parse --verify "${base_ref}^{commit}" 2>/dev/null)" \
  || fail "proof base is not a local commit: ${base_ref}"
base_ref="$base_commit"
git merge-base --is-ancestor "$base_ref" HEAD \
  || fail "proof base is not an ancestor of HEAD: ${base_ref}"

run_step() {
  local name="$1"; shift
  log "jankurai: $name"
  if ! "$@"; then
    warn "jankurai: $name failed"
    status=1
  fi
}

run_step "audit-ci (clean exact-head score)" bash ops/ci/score.sh

run_step "proof-routing" \
  "$JBIN" proof . --changed-from "$base_ref" \
    --out "${ARTIFACT_DIR}/proof-routing.json" --md "${ARTIFACT_DIR}/proof-routing.md"

run_step "proofbind" bash ops/ci/proofbind.sh

run_step "proofmark-rust" \
  "$JBIN" proofmark rust . --obligations "${ARTIFACT_DIR}/proofbind/obligations.json" \
    --out "${ARTIFACT_DIR}/proofmark/proofmark-receipt.json"

run_step "copy-code" \
  "$JBIN" copy-code . --json "${ARTIFACT_DIR}/copy-code.json" --md "${ARTIFACT_DIR}/copy-code.md"

run_step "security evidence" \
  "$JBIN" security run . --script tools/security-lane.sh --strict --profile release \
    --out "${ARTIFACT_DIR}/security/evidence.json"

run_step "language bad-behavior" bash ops/ci/language-bad-behavior.sh

run_step "rust-witness" \
  "$JBIN" rust witness build . --out "${ARTIFACT_DIR}/rust/witness-graph.json"

# authz-matrix / input-boundary / agent-tool-supply are audit detectors; the
# audit run below produces their evidence in the repo score JSON.
run_step "authz-matrix + input-boundary + agent-tool-supply (audit detectors)" \
  "$JBIN" audit . --mode advisory --full --no-score-history \
    --policy .jankurai/audit-policy.toml \
    --json "${ARTIFACT_DIR}/authz-matrix.json" --md "${ARTIFACT_DIR}/authz-matrix.md"

run_step "contract-drift" bash ops/ci/contract-drift.sh
run_step "release-readiness (script)" bash ops/ci/release-readiness.sh
run_step "cost-budget" bash ops/ci/cost-budget.sh
run_step "ux-qa" bash ops/ci/ux-qa.sh

[[ -z "$(git status --porcelain)" ]] || {
  warn "Jankurai proof mutated tracked or unignored evidence"
  status=1
}
exit "$status"
