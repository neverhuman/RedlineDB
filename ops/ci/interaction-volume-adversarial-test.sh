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
command -v nc >/dev/null 2>&1 || {
  printf 'fake-responder rejection test requires netcat\n' >&2
  exit 1
}
fake_body="$(jq -cn --arg commit "$source_commit" '{
  id:2,name:"interaction-volume-daily",source:"schedule",status:"running",ref:"main",
  web_url:"http://127.0.0.1/fake-job",commit:{id:$commit},
  pipeline:{id:1,project_id:3,ref:"main",sha:$commit},runner:{id:4}
}')"
fake_responder_pid=''
for offset in $(seq 0 9); do
  fake_port="$((20000 + ($$ % 20000) + offset))"
  {
    printf 'HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: %s\r\nConnection: close\r\n\r\n%s' \
      "${#fake_body}" "$fake_body" | nc -l 127.0.0.1 "$fake_port"
  } >/dev/null 2>&1 &
  fake_responder_pid=$!
  sleep 0.1
  kill -0 "$fake_responder_pid" >/dev/null 2>&1 && break
  wait "$fake_responder_pid" >/dev/null 2>&1 || true
  fake_responder_pid=''
done
[ -n "$fake_responder_pid" ] || {
  printf 'could not start fake CI responder\n' >&2
  exit 1
}
stop_fake_responder() {
  kill "$fake_responder_pid" >/dev/null 2>&1 || true
  wait "$fake_responder_pid" >/dev/null 2>&1 || true
}
trap stop_fake_responder EXIT
assert_daily_trigger_rejected fake_ci_responder \
  'daily certification blocked:' \
  CI=true CI_JOB_NAME=interaction-volume-daily CI_COMMIT_SHA="$source_commit" \
  CI_PIPELINE_SOURCE=schedule CI_PROJECT_PATH=jeryu/redline-core \
  CI_DEFAULT_BRANCH=main CI_COMMIT_BRANCH=main CI_COMMIT_REF_PROTECTED=true \
  CI_PIPELINE_ID=1 CI_JOB_ID=2 CI_PROJECT_ID=3 CI_RUNNER_ID=4 \
  CI_SERVER_URL="http://127.0.0.1:${fake_port}" \
  CI_PROJECT_URL="http://127.0.0.1:${fake_port}/jeryu/redline-core" \
  CI_PIPELINE_URL="http://127.0.0.1:${fake_port}/jeryu/redline-core/-/pipelines/1" \
  CI_API_V4_URL="http://127.0.0.1:${fake_port}/api/v4" CI_JOB_TOKEN=fabricated-token
kill -0 "$fake_responder_pid" >/dev/null 2>&1 || {
  printf 'daily entrypoint contacted the untrusted fake CI responder\n' >&2
  exit 1
}
stop_fake_responder
trap - EXIT

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
  if [ -s "$receipt/manifest.json" ]; then
    cleanup_sha="$(sha256sum "$receipt/cleanup.json" | awk '{print $1}')"
    jq -e --arg cleanup_sha "$cleanup_sha" '
      .schema_version == "redline.interaction-volume-cert/v4" and
      .cleanup_receipt == "cleanup.json" and
      .cleanup_receipt_sha256 == $cleanup_sha
    ' "$receipt/manifest.json" >/dev/null
  fi
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
