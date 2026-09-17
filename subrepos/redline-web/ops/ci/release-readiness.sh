#!/usr/bin/env bash
# Release-readiness lane: assert the release control surface exists (version
# source, changelog, release + rollback docs, evidence paths) and emit a
# receipt.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
ensure_artifacts

log "release-readiness: validating release evidence surface"
if ! has jq; then
  missing_tool jq "release readiness receipt"
  exit 0
fi

required_files=(
  CHANGELOG.md
  docs/release.md
  docs/testing.md
  docs/operations.md
  agent/cost-budget.toml
  apps/api/Cargo.toml
)
required_terms=(
  'bash ops/ci/pr-ci.sh'
  'ops/ci/security.sh'
  'ops/ci/release-readiness.sh'
  'target/jankurai'
  'rollback'
  'version'
)
missing_files=()
missing_terms=()
for path in "${required_files[@]}"; do
  [[ -f "$path" ]] || missing_files+=("$path")
done
for term in "${required_terms[@]}"; do
  if ! grep -Fq -- "$term" docs/release.md docs/testing.md 2>/dev/null; then
    missing_terms+=("$term")
  fi
done

required_files_json="$(json_array "${required_files[@]}")"
missing_files_json="$(json_array "${missing_files[@]}")"
missing_terms_json="$(json_array "${missing_terms[@]}")"
ok=true
if [[ "${#missing_files[@]}" -ne 0 || "${#missing_terms[@]}" -ne 0 ]]; then
  ok=false
fi
jq -n \
  --argjson ok "$ok" \
  --argjson required_files "$required_files_json" \
  --argjson missing_files "$missing_files_json" \
  --argjson missing_terms "$missing_terms_json" \
  '{
    ok: $ok,
    version_source: "apps/api/Cargo.toml",
    required_files: $required_files,
    missing_files: $missing_files,
    missing_terms: $missing_terms,
    artifact_paths: [
      "target/jankurai/release-readiness.json",
      "target/jankurai/cost-budget.json",
      "target/jankurai/security/evidence.json"
    ]
  }' >target/jankurai/release-readiness.json

if [[ "$ok" != true ]]; then
  fail "release readiness missing evidence: files=${missing_files[*]:-none} terms=${missing_terms[*]:-none}"
fi

log "release-readiness: complete"
