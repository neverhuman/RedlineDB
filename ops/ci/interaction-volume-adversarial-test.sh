#!/usr/bin/env bash
# Prove untrusted triggers, stale Docker evidence, endpoint substitution, and PostgreSQL cap
# overshoot fail closed.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

source_commit="$(git rev-parse --verify HEAD)"

assert_daily_trigger_rejected() {
  case_name="${1:?case name required}"
  expected="${2:?expected diagnostic required}"
  shift 2
  if output="$(env -i PATH="$PATH" HOME="${HOME:-/tmp}" "$@" \
    bash ops/ci/interaction-volume-ci-entrypoint.sh daily 2>&1)"; then
    printf 'untrusted daily trigger %s unexpectedly succeeded\n' "$case_name" >&2
    exit 1
  fi
  grep -F "$expected" <<<"$output" >/dev/null || {
    printf 'untrusted daily trigger %s lacked expected diagnostic %s:\n%s\n' \
      "$case_name" "$expected" "$output" >&2
    exit 1
  }
}

assert_daily_trigger_rejected non_ci \
  'interaction-volume CI entrypoint requires CI=true'
assert_daily_trigger_rejected non_schedule \
  'daily certificate requires a scheduled pipeline' \
  CI=true CI_JOB_NAME=interaction-volume-daily CI_COMMIT_SHA="$source_commit" \
  CI_PIPELINE_SOURCE=web
assert_daily_trigger_rejected fork_project \
  'daily certificate rejects non-canonical project' \
  CI=true CI_JOB_NAME=interaction-volume-daily CI_COMMIT_SHA="$source_commit" \
  CI_PIPELINE_SOURCE=schedule CI_PROJECT_PATH=customer/redline-core
assert_daily_trigger_rejected unprotected_ref \
  'daily certificate requires a protected commit ref' \
  CI=true CI_JOB_NAME=interaction-volume-daily CI_COMMIT_SHA="$source_commit" \
  CI_PIPELINE_SOURCE=schedule CI_PROJECT_PATH=jeryu/redline-core \
  CI_DEFAULT_BRANCH=main CI_COMMIT_BRANCH=main CI_COMMIT_REF_PROTECTED=false
assert_daily_trigger_rejected fabricated_canonical_job \
  'live CI job-token attestation request failed' \
  CI=true CI_JOB_NAME=interaction-volume-daily CI_COMMIT_SHA="$source_commit" \
  CI_PIPELINE_SOURCE=schedule CI_PROJECT_PATH=jeryu/redline-core \
  CI_DEFAULT_BRANCH=main CI_COMMIT_BRANCH=main CI_COMMIT_REF_PROTECTED=true \
  CI_PIPELINE_ID=1 CI_JOB_ID=2 CI_PROJECT_ID=3 CI_RUNNER_ID=4 \
  CI_SERVER_URL=http://127.0.0.1:9 \
  CI_PROJECT_URL=http://127.0.0.1:9/jeryu/redline-core \
  CI_PIPELINE_URL=http://127.0.0.1:9/jeryu/redline-core/-/pipelines/1 \
  CI_API_V4_URL=http://127.0.0.1:9/api/v4 CI_JOB_TOKEN=fabricated-token

export CI=true

for case_name in stale_container endpoint_mismatch postgres_storage_overshoot; do
  REDLINEDB_CERT_ADVERSARIAL_TEST_CASE="$case_name" \
    bash ops/ci/interaction-volume-cert.sh smoke
  receipt="target/ci/interaction-volume/smoke-adversarial-${case_name}"
  jq -e '
    .schema_version == "redline.interaction-volume-cleanup/v1" and
    .child_stopped and
    .postgres_schema_cleanup_verified and
    .container_removed and
    .runtime_removed and
    (.container_id | length == 64)
  ' "$receipt/cleanup.json" >/dev/null
done

if find target/ci/interaction-volume/runtime -mindepth 1 -maxdepth 1 -print -quit \
  | grep -q .; then
  printf 'adversarial certification left runtime directories\n' >&2
  exit 1
fi
if docker ps -a --filter name=redline-interaction-cert --format '{{.Names}}' | grep -q .; then
  printf 'adversarial certification left Docker containers\n' >&2
  exit 1
fi

printf 'interaction-volume adversarial rejection receipts verified\n'
