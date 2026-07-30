#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
validator="$repo_root/ops/ci/validate-jankurai-evidence.sh"
scratch="$(mktemp -d "$repo_root/target/jankurai-evidence-hostile.XXXXXX")"
cleanup() {
  if [[ "${JANKURAI_EVIDENCE_KEEP_SCRATCH:-0}" == 1 ]]; then
    printf 'retained hostile scratch: %s\n' "$scratch" >&2
  else
    rm -rf -- "$scratch"
  fi
}
trap cleanup EXIT

head_sha=0123456789abcdef0123456789abcdef01234567
proofbind="$scratch/proofbind.json"
proofmark="$scratch/proofmark.json"

jq -n --arg head "$head_sha" '{
  generated_at: "fixture",
  git_head: $head,
  mode: "required",
  obligations: [{
    obligation_id: "obligation:HLT-023:fixture",
    path: "tests/jankurai_evidence_hostile.sh",
    receipt_paths: ["target/jankurai/proof-receipts/final/proofmark-rust.json"],
    repair_task: "none",
    required_lanes: ["proofmark-rust"],
    required_receipt_kinds: ["proof-receipt", "proofmark", "negative-behavior-proof"],
    risk_tags: ["changed_behavior"],
    rule_ids: ["HLT-023-INPUT-BOUNDARY-GAP"],
    satisfied: true,
    severity: "high",
    status: "satisfied",
    surface_id: "surface:input_boundary:fixture",
    surface_type: "input_boundary",
    symbol: "fixture"
  }],
  repo_root: ".",
  schema_version: "1.0.0",
  standard_version: "0.7.0",
  summary: {
    changed_surface_count: 1,
    high_or_critical_missing: 0,
    missing: 0,
    satisfied: 1,
    total: 1,
    verdict: "pass"
  }
}' >"$proofbind"

jq -n --arg head "$head_sha" '{
  artifacts: [
    "target/jankurai/proofmark/proofmark-receipt.json",
    "target/jankurai/proofmark/proofmark.md"
  ],
  auditor_version: "0.7.0",
  changed_paths: ["tests/jankurai_evidence_hostile.sh"],
  command: "jankurai proofmark rust",
  dirty_worktree: false,
  elapsed_ms: 1,
  exit_code: 0,
  extensions: {
    proofmark: {
      changed_units: [{
        changed_lines: [1],
        coverage_status: "pass",
        covered_changed_lines: [1],
        path: "tests/jankurai_evidence_hostile.sh",
        uncovered_changed_lines: [],
        unit: "jankurai_evidence_hostile"
      }],
      coverage: {
        changed_line_count: 1,
        covered_changed_line_count: 1,
        source: "target/jankurai/evidence-contract/lcov.info",
        status: "pass",
        uncovered_changed_line_count: 0
      },
      mutation: {
        killed: 10,
        source: "target/jankurai/evidence-contract/mutation.json",
        status: "pass",
        survived: 0,
        timeout: 0
      },
      obligation_results: [{
        coverage_status: "pass",
        evidence: ["target/jankurai/evidence-contract/negative-proof.json"],
        mutation_status: "pass",
        negative_proof_status: "present",
        obligation_id: "obligation:HLT-023:fixture",
        path: "tests/jankurai_evidence_hostile.sh",
        required_lanes: ["proofmark-rust"],
        residual_risk: [],
        rule_ids: ["HLT-023-INPUT-BOUNDARY-GAP"],
        status: "pass"
      }],
      satisfied_obligations: ["obligation:HLT-023:fixture"],
      schema_version: "1.0.0",
      summary: {
        changed_units: 1,
        review_obligations: 0,
        satisfied_obligations: 1,
        total_obligations: 1,
        verdict: "pass"
      }
    }
  },
  generated_at: "fixture",
  git_head: $head,
  lane: "proofmark-rust",
  receipt_id: "proofmark-rust-fixture",
  repo_root: ".",
  rules_covered: [{
    rule_id: "HLT-023-INPUT-BOUNDARY-GAP",
    status: "covered"
  }],
  schema_version: "1.0.0",
  standard_version: "0.7.0"
}' >"$proofmark"

"$validator" proofbind "$proofbind" "$head_sha"
"$validator" proofmark "$proofmark" "$head_sha"

killed=0
reject_mutation() {
  local kind="$1" name="$2" filter="$3" source="$4"
  local mutant="$scratch/${kind}-${name}.json"
  jq "$filter" "$source" >"$mutant"
  if "$validator" "$kind" "$mutant" "$head_sha" >/dev/null 2>&1; then
    printf 'validator accepted %s mutant: %s\n' "$kind" "$name" >&2
    exit 1
  fi
  killed=$((killed + 1))
}

reject_mutation proofbind extra-field '.unexpected = true' "$proofbind"
reject_mutation proofbind wrong-head '.git_head = "ffffffffffffffffffffffffffffffffffffffff"' "$proofbind"
reject_mutation proofbind vacuous '.obligations = [] | .summary = {changed_surface_count:0,high_or_critical_missing:0,missing:0,satisfied:0,total:0,verdict:"pass"}' "$proofbind"
reject_mutation proofbind review-verdict '.summary.verdict = "review"' "$proofbind"
reject_mutation proofbind missing-obligation '.obligations[0].satisfied = false | .obligations[0].status = "missing" | .summary.satisfied = 0 | .summary.missing = 1 | .summary.high_or_critical_missing = 1 | .summary.verdict = "review"' "$proofbind"

reject_mutation proofmark extra-field '.extensions.proofmark.unexpected = true' "$proofmark"
reject_mutation proofmark wrong-head '.git_head = "ffffffffffffffffffffffffffffffffffffffff"' "$proofmark"
reject_mutation proofmark invalid-rule-status '.rules_covered[0].status = "pass"' "$proofmark"
reject_mutation proofmark unavailable-coverage '.extensions.proofmark.coverage.source = "unavailable" | .extensions.proofmark.coverage.status = "unavailable"' "$proofmark"
reject_mutation proofmark unavailable-mutation '.extensions.proofmark.mutation.source = "unavailable" | .extensions.proofmark.mutation.status = "unavailable"' "$proofmark"
reject_mutation proofmark missing-negative '.extensions.proofmark.obligation_results[0].negative_proof_status = "missing"' "$proofmark"
reject_mutation proofmark review-verdict '.extensions.proofmark.summary.verdict = "review" | .extensions.proofmark.summary.review_obligations = 1' "$proofmark"

mutation_out="${JANKURAI_EVIDENCE_MUTATION_OUT:-$repo_root/target/jankurai/evidence-contract/mutation.json}"
mkdir -p "$(dirname "$mutation_out")"
jq -n --argjson killed "$killed" '{
  killed: $killed,
  survived: 0,
  timeout: 0,
  unviable: 0
}' >"$mutation_out"

printf 'jankurai evidence contract hostiles passed: killed=%s survived=0\n' "$killed"
