#!/usr/bin/env bash
# Runtime CI boundary for the owned-Docker interaction-volume jobs.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

mode="${1:-}"
case "$mode" in
  smoke) expected_job='interaction-volume-smoke' ;;
  daily) expected_job='interaction-volume-daily' ;;
  *)
    printf 'usage: %s {smoke|daily}\n' "$0" >&2
    exit 2
    ;;
esac

[ "${CI:-}" = true ] || {
  printf 'interaction-volume CI entrypoint requires CI=true\n' >&2
  exit 2
}
[ "${CI_JOB_NAME:-}" = "$expected_job" ] || {
  printf 'CI job name %s does not match expected %s\n' "${CI_JOB_NAME:-missing}" "$expected_job" >&2
  exit 2
}
source_commit="$(git rev-parse --verify HEAD)"
[ "${CI_COMMIT_SHA:-}" = "$source_commit" ] || {
  printf 'CI commit %s differs from checked-out source %s\n' \
    "${CI_COMMIT_SHA:-missing}" "$source_commit" >&2
  exit 1
}

command -v docker >/dev/null 2>&1 || {
  printf 'CI image lacks the Docker CLI\n' >&2
  exit 2
}
docker version --format '{{.Client.Version}} {{.Server.Version}}' >/dev/null
docker info --format '{{json .ID}}' | jq -e 'type == "string" and length > 0' >/dev/null

export REDLINEDB_CERT_ALLOW_IMAGE_PULL=1
if [ "$mode" = daily ]; then
  case "${CI_PIPELINE_SOURCE:-}" in
    schedule|web|merge_request_event) ;;
    *)
      printf 'daily certificate rejects CI pipeline source %s\n' \
        "${CI_PIPELINE_SOURCE:-missing}" >&2
      exit 2
      ;;
  esac
  for variable in CI_PIPELINE_ID CI_JOB_ID CI_PROJECT_PATH CI_PIPELINE_URL; do
    [ -n "${!variable:-}" ] || {
      printf 'daily certificate lacks runtime CI field %s\n' "$variable" >&2
      exit 2
    }
  done
  case "$CI_PIPELINE_ID:$CI_JOB_ID" in
    *[!0-9:]*|0:*|*:0)
      printf 'daily certificate requires nonzero numeric pipeline/job ids\n' >&2
      exit 2
      ;;
  esac
  trigger_dir='target/ci/interaction-volume/trigger'
  mkdir -p "$trigger_dir"
  trigger_file="$trigger_dir/${CI_PIPELINE_ID}-${CI_JOB_ID}.json"
  scheduled=false
  [ "$CI_PIPELINE_SOURCE" = schedule ] && scheduled=true
  jq -n \
    --arg schema 'redline.interaction-volume-ci-trigger/v1' \
    --arg pipeline_source "$CI_PIPELINE_SOURCE" \
    --arg pipeline_id "$CI_PIPELINE_ID" \
    --arg job_id "$CI_JOB_ID" \
    --arg job_name "$CI_JOB_NAME" \
    --arg source_commit "$CI_COMMIT_SHA" \
    --arg project_path "$CI_PROJECT_PATH" \
    --arg pipeline_url "$CI_PIPELINE_URL" \
    --argjson scheduled "$scheduled" \
    '{schema_version:$schema,pipeline_source:$pipeline_source,pipeline_id:$pipeline_id,
      job_id:$job_id,job_name:$job_name,source_commit:$source_commit,
      project_path:$project_path,pipeline_url:$pipeline_url,scheduled:$scheduled}' \
    >"${trigger_file}.tmp"
  mv "${trigger_file}.tmp" "$trigger_file"
  export REDLINEDB_INTERACTION_TRIGGER_EVIDENCE_FILE="$trigger_file"
fi

exec bash ops/ci/interaction-volume-cert.sh "$mode"
