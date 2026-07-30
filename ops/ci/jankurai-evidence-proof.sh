#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"
cd "$repo_root"

for tool in cargo jq rustc sha256sum stat tee wc; do
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
raw_coverage="$evidence_root/llvm-coverage.json"
coverage="$evidence_root/coverage.json"
mutation="$evidence_root/mutation.json"
negative="$evidence_root/negative-proof.json"
hostile_log="$evidence_root/hostile.log"
mkdir -p "$evidence_root"

log "jankurai evidence: closed-schema and semantic mutation hostiles"
JANKURAI_EVIDENCE_MUTATION_OUT="$mutation" \
  bash tests/jankurai_evidence_hostile.sh | tee "$hostile_log"

log "jankurai evidence: real Rust coverage"
cargo llvm-cov --locked --workspace --no-report

shopt -s nullglob
ci_contract_candidates=(target/llvm-cov-target/debug/deps/ci_fail_closed-*)
manifest_contract_candidates=(
  target/llvm-cov-target/debug/deps/release_manifest_integrity-*
)
shopt -u nullglob
ci_contract_bins=()
manifest_contract_bins=()
for candidate in "${ci_contract_candidates[@]}"; do
  [[ -f "$candidate" && -x "$candidate" ]] && ci_contract_bins+=("$candidate")
done
for candidate in "${manifest_contract_candidates[@]}"; do
  [[ -f "$candidate" && -x "$candidate" ]] &&
    manifest_contract_bins+=("$candidate")
done
[[ "${#ci_contract_bins[@]}" == 1 &&
  "${#manifest_contract_bins[@]}" == 1 ]] || {
  fail "expected one executable object for each Jankurai integration contract"
}

llvm_bin="$(dirname "$(rustc --print target-libdir)")/bin"
llvm_cov="$llvm_bin/llvm-cov"
profdata=target/llvm-cov-target/redline-testing.profdata
[[ -x "$llvm_cov" && -f "$profdata" && ! -L "$profdata" &&
  -s "$profdata" ]] || {
  fail "rustc-matched llvm-cov or merged Redline Testing profile is unavailable"
}
for object in "${ci_contract_bins[0]}" "${manifest_contract_bins[0]}"; do
  [[ ! -L "$object" && "$(stat -c '%h' "$object")" == 1 ]] || {
    fail "instrumented integration-test object is aliased or linked: $object"
  }
done
"$llvm_cov" export "${ci_contract_bins[0]}" \
  -object "${manifest_contract_bins[0]}" \
  -instr-profile="$profdata" >"$raw_coverage"

ci_contract_lines="$(wc -l <tests/ci_fail_closed.rs)"
manifest_contract_lines="$(wc -l <tests/release_manifest_integrity.rs)"
jq \
  --argjson ci_contract_lines "$ci_contract_lines" \
  --argjson manifest_contract_lines "$manifest_contract_lines" \
  '
  def source_line_count:
    if (.filename | endswith("/tests/ci_fail_closed.rs")) then
      $ci_contract_lines
    elif (.filename | endswith("/tests/release_manifest_integrity.rs")) then
      $manifest_contract_lines
    else
      error("unrecognized proof coverage source")
    end;
  def normalized_file:
    ([.segments[] | select(.[3] == true) | .[0]] | unique) as $executable
    | ([.segments[] | select(.[3] == true and .[2] > 0) | .[0]] | unique) as $hit
    | source_line_count as $last
    | {
        filename,
        covered_lines: [
          range(1; $last + 1) as $line
          | select(
              ($executable | index($line)) == null
              or ($hit | index($line)) != null
            )
          | $line
        ]
      };
  {
    files: [
      .data[].files[]
      | select(
          (.filename | endswith("/tests/ci_fail_closed.rs"))
          or (.filename | endswith("/tests/release_manifest_integrity.rs"))
        )
      | normalized_file
    ]
  }
  ' "$raw_coverage" >"$coverage"
jq -e '
  (.files | length) == 2
  and ([.files[].filename | select(endswith("/tests/ci_fail_closed.rs"))]
    | length) == 1
  and ([.files[].filename
    | select(endswith("/tests/release_manifest_integrity.rs"))]
    | length) == 1
  and all(.files[]; (.covered_lines | length) > 0)
' "$coverage" >/dev/null

# Proofmark has no non-executable-line state. The normalized generic report
# preserves every real zero-count executable line as uncovered while treating
# comments, imports, blanks, and macro continuations without an LLVM segment
# as not applicable.
for artifact in "$raw_coverage" "$coverage" "$mutation" "$hostile_log"; do
  [[ -f "$artifact" && ! -L "$artifact" && -s "$artifact" ]] || {
    fail "Jankurai evidence artifact is missing, aliased, or empty: $artifact"
  }
  [[ "$(stat -c '%h' "$artifact")" == 1 ]] || {
    fail "Jankurai evidence artifact is not single-link: $artifact"
  }
done

coverage_sha256="$(sha256sum "$coverage" | awk '{print $1}')"
raw_coverage_sha256="$(sha256sum "$raw_coverage" | awk '{print $1}')"
mutation_sha256="$(sha256sum "$mutation" | awk '{print $1}')"
hostile_sha256="$(sha256sum "$hostile_log" | awk '{print $1}')"
validator_sha256="$(sha256sum ops/ci/validate-jankurai-evidence.sh | awk '{print $1}')"
hostile_source_sha256="$(sha256sum tests/jankurai_evidence_hostile.sh | awk '{print $1}')"

jq -n \
  --arg commit "$commit" \
  --arg tree "$tree" \
  --arg coverage "$coverage" \
  --arg coverage_sha256 "$coverage_sha256" \
  --arg raw_coverage "$raw_coverage" \
  --arg raw_coverage_sha256 "$raw_coverage_sha256" \
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
      artifact_sha256: $coverage_sha256,
      raw_artifact: $raw_coverage,
      raw_artifact_sha256: $raw_coverage_sha256,
      normalization: "non-executable lines are not applicable; zero-count executable lines remain uncovered"
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
  and (.coverage.raw_artifact_sha256 | test("^[0-9a-f]{64}$"))
  and .coverage.normalization ==
    "non-executable lines are not applicable; zero-count executable lines remain uncovered"
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
