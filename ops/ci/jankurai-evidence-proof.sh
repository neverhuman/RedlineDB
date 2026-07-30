#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"
cd "$repo_root"

for tool in cargo jq sha256sum stat tee; do
  has "$tool" || fail "missing required Jankurai evidence tool: $tool"
done
actual_llvm_cov="$(cargo llvm-cov --version)"
[[ "$actual_llvm_cov" == "cargo-llvm-cov 0.8.7" ]] || {
  fail "expected cargo-llvm-cov 0.8.7, got: $actual_llvm_cov"
}

commit="$(git rev-parse --verify 'HEAD^{commit}')"
tree="$(git rev-parse --verify 'HEAD^{tree}')"
[[ "$commit" =~ ^[0-9a-f]{40}$ && "$tree" =~ ^[0-9a-f]{40}$ ]] || {
  fail "Jankurai evidence requires full commit and tree identities"
}
[[ -z "$(git status --porcelain --untracked-files=no)" ]] || {
  fail "Jankurai evidence requires a clean committed source tree"
}

evidence_root=target/jankurai/evidence-contract
coverage="$evidence_root/lcov.info"
mutation="$evidence_root/mutation.json"
negative="$evidence_root/negative-proof.json"
hostile_log="$evidence_root/hostile.log"
mkdir -p "$evidence_root"

log "jankurai evidence: closed-schema and semantic mutation hostiles"
JANKURAI_EVIDENCE_MUTATION_OUT="$mutation" \
  bash tests/jankurai_evidence_hostile.sh | tee "$hostile_log"

log "jankurai evidence: real Rust coverage"
CARGO_TARGET_DIR="$repo_root/target/llvm-cov-target" \
  cargo llvm-cov --locked --workspace --lcov --output-path "$coverage"

for artifact in "$coverage" "$mutation" "$hostile_log"; do
  [[ -f "$artifact" && ! -L "$artifact" && -s "$artifact" ]] || {
    fail "Jankurai evidence artifact is missing, aliased, or empty: $artifact"
  }
  [[ "$(stat -c '%h' "$artifact")" == 1 ]] || {
    fail "Jankurai evidence artifact is not single-link: $artifact"
  }
done

coverage_sha256="$(sha256sum "$coverage" | awk '{print $1}')"
mutation_sha256="$(sha256sum "$mutation" | awk '{print $1}')"
hostile_sha256="$(sha256sum "$hostile_log" | awk '{print $1}')"
validator_sha256="$(sha256sum ops/ci/validate-jankurai-evidence.sh | awk '{print $1}')"
hostile_source_sha256="$(sha256sum tests/jankurai_evidence_hostile.sh | awk '{print $1}')"

jq -n \
  --arg commit "$commit" \
  --arg tree "$tree" \
  --arg coverage "$coverage" \
  --arg coverage_sha256 "$coverage_sha256" \
  --arg mutation "$mutation" \
  --arg mutation_sha256 "$mutation_sha256" \
  --arg hostile_log "$hostile_log" \
  --arg hostile_sha256 "$hostile_sha256" \
  --arg validator_sha256 "$validator_sha256" \
  --arg hostile_source_sha256 "$hostile_source_sha256" \
  --slurpfile mutations "$mutation" \
  '{
    schema: "redline-testing.jankurai-negative-proof/v1",
    source_commit: $commit,
    source_tree: $tree,
    source_state: "clean-committed",
    rule_ids: [
      "HLT-023-INPUT-BOUNDARY-GAP",
      "HLT-024-AGENT-TOOL-SUPPLY-GAP"
    ],
    source_files: [
      {path: "ops/ci/validate-jankurai-evidence.sh", sha256: $validator_sha256},
      {path: "tests/jankurai_evidence_hostile.sh", sha256: $hostile_source_sha256}
    ],
    hostile: {
      status: "pass",
      log: $hostile_log,
      log_sha256: $hostile_sha256
    },
    coverage: {
      status: "pass",
      artifact: $coverage,
      artifact_sha256: $coverage_sha256
    },
    mutation: {
      status: "pass",
      artifact: $mutation,
      artifact_sha256: $mutation_sha256,
      killed: $mutations[0].killed,
      survived: $mutations[0].survived,
      timeout: $mutations[0].timeout,
      unviable: $mutations[0].unviable
    },
    status: "pass"
  }' >"$negative"

jq -e --arg commit "$commit" --arg tree "$tree" '
  (keys | sort) == [
    "coverage", "hostile", "mutation", "rule_ids", "schema", "source_commit",
    "source_files", "source_state", "source_tree", "status"
  ]
  and .schema == "redline-testing.jankurai-negative-proof/v1"
  and .source_commit == $commit
  and .source_tree == $tree
  and .source_state == "clean-committed"
  and .rule_ids == [
    "HLT-023-INPUT-BOUNDARY-GAP",
    "HLT-024-AGENT-TOOL-SUPPLY-GAP"
  ]
  and (.source_files | map(.path)) == [
    "ops/ci/validate-jankurai-evidence.sh",
    "tests/jankurai_evidence_hostile.sh"
  ]
  and (all(.source_files[];
    (.sha256 | test("^[0-9a-f]{64}$"))))
  and .hostile.status == "pass"
  and (.hostile.log_sha256 | test("^[0-9a-f]{64}$"))
  and .coverage.status == "pass"
  and (.coverage.artifact_sha256 | test("^[0-9a-f]{64}$"))
  and .mutation.status == "pass"
  and .mutation.killed >= 10
  and .mutation.survived == 0
  and .mutation.timeout == 0
  and .mutation.unviable == 0
  and (.mutation.artifact_sha256 | test("^[0-9a-f]{64}$"))
  and .status == "pass"
' "$negative" >/dev/null

while IFS=$'\t' read -r path expected; do
  [[ "$(sha256sum "$path" | awk '{print $1}')" == "$expected" ]] || {
    fail "Jankurai negative-proof digest mismatch: $path"
  }
done < <(jq -r '
  .source_files[] | [.path, .sha256] | @tsv
  ' "$negative")

printf 'jankurai evidence proof ok: commit=%s tree=%s killed=%s\n' \
  "$commit" "$tree" "$(jq -er '.mutation.killed' "$negative")"
