#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'usage: %s {proofbind|proofmark} RECEIPT EXPECTED_HEAD\n' "$0" >&2
  exit 2
}

[[ "$#" == 3 ]] || usage
kind="$1"
receipt="$2"
expected_head="$3"

[[ "$expected_head" =~ ^[0-9a-f]{40}$ ]] || {
  printf 'expected head must be one full lowercase Git object id\n' >&2
  exit 1
}
[[ -f "$receipt" && ! -L "$receipt" ]] || {
  printf 'Jankurai receipt is missing or aliased: %s\n' "$receipt" >&2
  exit 1
}
[[ "$(stat -c '%h' "$receipt")" == 1 ]] || {
  printf 'Jankurai receipt must be a single-link regular file: %s\n' "$receipt" >&2
  exit 1
}

validate_proofbind() {
  jq -e --arg head "$expected_head" '
    (keys | sort) == [
      "generated_at", "git_head", "mode", "obligations", "repo_root",
      "schema_version", "standard_version", "summary"
    ]
    and .schema_version == "1.0.0"
    and .standard_version == "0.7.0"
    and .git_head == $head
    and .repo_root == "."
    and .mode == "required"
    and (.generated_at | type) == "string"
    and (.obligations | type) == "array"
    and (.obligations | length) > 0
    and (.obligations | length) ==
      ([.obligations[].obligation_id] | unique | length)
    and (all(.obligations[];
      (keys | sort) == [
        "obligation_id", "path", "receipt_paths", "repair_task",
        "required_lanes", "required_receipt_kinds", "risk_tags", "rule_ids",
        "satisfied", "severity", "status", "surface_id", "surface_type",
        "symbol"
      ]
      and (.obligation_id | type) == "string"
      and (.obligation_id | length) > 0
      and (.surface_id | type) == "string"
      and (.surface_id | length) > 0
      and (.path | type) == "string"
      and (.path | length) > 0
      and (.symbol | type) == "string"
      and (.surface_type | type) == "string"
      and (.severity | type) == "string"
      and (.risk_tags | type) == "array"
      and (.rule_ids | type) == "array"
      and (.rule_ids | length) > 0
      and (.required_lanes | type) == "array"
      and (.required_lanes | length) > 0
      and (.required_receipt_kinds | type) == "array"
      and (.required_receipt_kinds | length) > 0
      and (.receipt_paths | type) == "array"
      and (.receipt_paths | length) > 0
      and (.repair_task | type) == "string"
      and .satisfied == true
      and .status == "satisfied"))
    and (.summary | keys | sort) == [
      "changed_surface_count", "high_or_critical_missing", "missing",
      "satisfied", "total", "verdict"
    ]
    and .summary.total == (.obligations | length)
    and .summary.total > 0
    and .summary.changed_surface_count == .summary.total
    and .summary.satisfied == .summary.total
    and .summary.missing == 0
    and .summary.high_or_critical_missing == 0
    and .summary.verdict == "pass"
  ' "$receipt" >/dev/null
}

validate_proofmark() {
  jq -e --arg head "$expected_head" '
    (keys | sort) == [
      "artifacts", "auditor_version", "changed_paths", "command",
      "dirty_worktree", "elapsed_ms", "exit_code", "extensions",
      "generated_at", "git_head", "lane", "receipt_id", "repo_root",
      "rules_covered", "schema_version", "standard_version"
    ]
    and .schema_version == "1.0.0"
    and .standard_version == "0.7.0"
    and .auditor_version == "0.7.0"
    and .git_head == $head
    and .repo_root == "."
    and .lane == "proofmark-rust"
    and .command == "jankurai proofmark rust"
    and .exit_code == 0
    and .dirty_worktree == false
    and (.elapsed_ms | type) == "number"
    and (.generated_at | type) == "string"
    and (.receipt_id | type) == "string"
    and (.receipt_id | length) > 0
    and (.changed_paths | type) == "array"
    and (.changed_paths | length) > 0
    and (.changed_paths | length) == (.changed_paths | unique | length)
    and (.artifacts | type) == "array"
    and (.artifacts | length) > 0
    and (.rules_covered | type) == "array"
    and (.rules_covered | length) > 0
    and (.rules_covered | length) ==
      ([.rules_covered[].rule_id] | unique | length)
    and (all(.rules_covered[];
      (keys | sort) == ["rule_id", "status"]
      and (.rule_id | type) == "string"
      and (.rule_id | length) > 0
      and .status == "pass"))
    and (.extensions | keys) == ["proofmark"]
    and (.extensions.proofmark | keys | sort) == [
      "changed_units", "coverage", "mutation", "obligation_results",
      "satisfied_obligations", "schema_version", "summary"
    ]
    and .extensions.proofmark.schema_version == "1.0.0"
    and (.extensions.proofmark.changed_units | type) == "array"
    and (.extensions.proofmark.changed_units | length) > 0
    and (all(.extensions.proofmark.changed_units[];
      (keys | sort) == [
        "changed_lines", "coverage_status", "covered_changed_lines", "path",
        "uncovered_changed_lines", "unit"
      ]
      and (.path | type) == "string"
      and (.path | length) > 0
      and (.unit | type) == "string"
      and (.changed_lines | type) == "array"
      and (.changed_lines | length) > 0
      and (.covered_changed_lines | sort) == (.changed_lines | sort)
      and (.uncovered_changed_lines | type) == "array"
      and (.uncovered_changed_lines | length) == 0
      and .coverage_status == "pass"))
    and (.extensions.proofmark.coverage | keys | sort) == [
      "changed_line_count", "covered_changed_line_count", "source", "status",
      "uncovered_changed_line_count"
    ]
    and (.extensions.proofmark.coverage.source | type) == "string"
    and .extensions.proofmark.coverage.source != "unavailable"
    and .extensions.proofmark.coverage.status == "pass"
    and .extensions.proofmark.coverage.changed_line_count > 0
    and .extensions.proofmark.coverage.covered_changed_line_count ==
      .extensions.proofmark.coverage.changed_line_count
    and .extensions.proofmark.coverage.uncovered_changed_line_count == 0
    and (.extensions.proofmark.mutation | keys | sort) == [
      "killed", "source", "status", "survived", "timeout"
    ]
    and (.extensions.proofmark.mutation.source | type) == "string"
    and .extensions.proofmark.mutation.source != "unavailable"
    and .extensions.proofmark.mutation.status == "pass"
    and .extensions.proofmark.mutation.killed > 0
    and .extensions.proofmark.mutation.survived == 0
    and .extensions.proofmark.mutation.timeout == 0
    and (.extensions.proofmark.obligation_results | type) == "array"
    and (.extensions.proofmark.obligation_results | length) > 0
    and (.extensions.proofmark.obligation_results | length) ==
      ([.extensions.proofmark.obligation_results[].obligation_id] | unique | length)
    and (all(.extensions.proofmark.obligation_results[];
      (keys | sort) == [
        "coverage_status", "evidence", "mutation_status",
        "negative_proof_status", "obligation_id", "path", "required_lanes",
        "residual_risk", "rule_ids", "status"
      ]
      and (.obligation_id | type) == "string"
      and (.obligation_id | length) > 0
      and (.path | type) == "string"
      and (.required_lanes | type) == "array"
      and (.required_lanes | length) > 0
      and (.rule_ids | type) == "array"
      and (.rule_ids | length) > 0
      and (.evidence | type) == "array"
      and (.evidence | length) > 0
      and (.residual_risk | type) == "array"
      and (.residual_risk | length) == 0
      and .coverage_status == "pass"
      and .mutation_status == "pass"
      and .status == "pass"
      and (if ((.rule_ids | index("HLT-023-INPUT-BOUNDARY-GAP")) or
               (.rule_ids | index("HLT-024-AGENT-TOOL-SUPPLY-GAP")))
           then .negative_proof_status == "present"
           else (.negative_proof_status == "present" or
                 .negative_proof_status == "not_required")
           end)))
    and (.extensions.proofmark.satisfied_obligations | type) == "array"
    and (.extensions.proofmark.satisfied_obligations | sort) ==
      ([.extensions.proofmark.obligation_results[].obligation_id] | sort)
    and (.extensions.proofmark.summary | keys | sort) == [
      "changed_units", "review_obligations", "satisfied_obligations",
      "total_obligations", "verdict"
    ]
    and .extensions.proofmark.summary.changed_units ==
      (.extensions.proofmark.changed_units | length)
    and .extensions.proofmark.summary.total_obligations ==
      (.extensions.proofmark.obligation_results | length)
    and .extensions.proofmark.summary.satisfied_obligations ==
      .extensions.proofmark.summary.total_obligations
    and .extensions.proofmark.summary.review_obligations == 0
    and .extensions.proofmark.summary.verdict == "pass"
    and ([.extensions.proofmark.obligation_results[].rule_ids[]] | unique | sort) ==
      ([.rules_covered[].rule_id] | sort)
  ' "$receipt" >/dev/null
}

case "$kind" in
  proofbind) validate_proofbind ;;
  proofmark) validate_proofmark ;;
  *) usage ;;
esac
