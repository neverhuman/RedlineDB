#!/usr/bin/env bash
# Runtime CI boundary for the owned-Docker interaction-volume jobs.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

mode="${1:-}"
canonical_project_path='jeryu/redline-core'
canonical_branch='main'
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

command -v jq >/dev/null 2>&1 || {
  printf 'CI image lacks jq\n' >&2
  exit 2
}
if [ "$mode" = daily ]; then
  [ "${CI_PIPELINE_SOURCE:-}" = schedule ] || {
    printf 'daily certificate requires a scheduled pipeline, got %s\n' \
      "${CI_PIPELINE_SOURCE:-missing}" >&2
    exit 2
  }
  [ "${CI_PROJECT_PATH:-}" = "$canonical_project_path" ] || {
    printf 'daily certificate rejects non-canonical project %s\n' \
      "${CI_PROJECT_PATH:-missing}" >&2
    exit 2
  }
  if [ "${CI_DEFAULT_BRANCH:-}" != "$canonical_branch" ] || \
    [ "${CI_COMMIT_BRANCH:-}" != "$canonical_branch" ]; then
    printf 'daily certificate requires canonical branch %s\n' "$canonical_branch" >&2
    exit 2
  fi
  [ "${CI_COMMIT_REF_PROTECTED:-}" = true ] || {
    printf 'daily certificate requires a protected commit ref\n' >&2
    exit 2
  }
  for variable in CI_PIPELINE_ID CI_JOB_ID CI_PROJECT_ID CI_RUNNER_ID \
    CI_SERVER_URL CI_PROJECT_URL CI_PIPELINE_URL CI_API_V4_URL CI_JOB_TOKEN; do
    [ -n "${!variable:-}" ] || {
      printf 'daily certificate lacks runtime CI field %s\n' "$variable" >&2
      exit 2
    }
  done
  for variable in CI_PIPELINE_ID CI_JOB_ID CI_PROJECT_ID CI_RUNNER_ID; do
    case "${!variable}" in
      *[!0-9]*|0)
        printf 'daily certificate requires nonzero numeric %s\n' "$variable" >&2
        exit 2
        ;;
    esac
  done
  expected_project_url="${CI_SERVER_URL%/}/${canonical_project_path}"
  [ "$CI_PROJECT_URL" = "$expected_project_url" ] || {
    printf 'CI project URL %s differs from canonical %s\n' \
      "$CI_PROJECT_URL" "$expected_project_url" >&2
    exit 2
  }
  case "$CI_PIPELINE_URL" in
    "${CI_PROJECT_URL}/-/pipelines/${CI_PIPELINE_ID}") ;;
    *)
      printf 'CI pipeline URL is not bound to the canonical project and pipeline id\n' >&2
      exit 2
      ;;
  esac
  expected_api_url="${CI_SERVER_URL%/}/api/v4"
  [ "$CI_API_V4_URL" = "$expected_api_url" ] || {
    printf 'CI API URL %s differs from canonical %s\n' \
      "$CI_API_V4_URL" "$expected_api_url" >&2
    exit 2
  }
  case "$CI_JOB_TOKEN" in
    *[!A-Za-z0-9_.-]*|'')
      printf 'daily certificate received an invalid CI job token shape\n' >&2
      exit 2
      ;;
  esac
  command -v curl >/dev/null 2>&1 || {
    printf 'daily certificate requires curl for live CI job attestation\n' >&2
    exit 2
  }
  trigger_dir='target/ci/interaction-volume/trigger'
  mkdir -p "$trigger_dir"
  trigger_file="$trigger_dir/${CI_PIPELINE_ID}-${CI_JOB_ID}.json"
  umask 077
  if ! job_response_json="$(printf 'JOB-TOKEN: %s\n' "$CI_JOB_TOKEN" | \
    curl --fail --silent --show-error --max-time 15 --header @- \
      "${CI_API_V4_URL}/job")"; then
    printf 'live CI job-token attestation request failed\n' >&2
    exit 1
  fi
  expected_job_url="${CI_PROJECT_URL}/-/jobs/${CI_JOB_ID}"
  jq -e \
    --argjson job_id "$CI_JOB_ID" \
    --argjson pipeline_id "$CI_PIPELINE_ID" \
    --argjson project_id "$CI_PROJECT_ID" \
    --argjson runner_id "$CI_RUNNER_ID" \
    --arg job_name "$CI_JOB_NAME" \
    --arg source_commit "$CI_COMMIT_SHA" \
    --arg branch "$canonical_branch" \
    --arg job_url "$expected_job_url" '
      .id == $job_id and .name == $job_name and .source == "schedule" and
      .status == "running" and .ref == $branch and .web_url == $job_url and
      .commit.id == $source_commit and
      .pipeline.id == $pipeline_id and .pipeline.project_id == $project_id and
      .pipeline.ref == $branch and .pipeline.sha == $source_commit and
      .runner.id == $runner_id
    ' <<<"$job_response_json" >/dev/null || {
    printf 'live CI job-token attestation does not match the scheduled canonical job\n' >&2
    exit 1
  }
  unset job_response_json
  jq -n \
    --arg schema 'redline.interaction-volume-ci-trigger/v2' \
    --arg pipeline_source "$CI_PIPELINE_SOURCE" \
    --arg pipeline_id "$CI_PIPELINE_ID" \
    --arg job_id "$CI_JOB_ID" \
    --arg project_id "$CI_PROJECT_ID" \
    --arg runner_id "$CI_RUNNER_ID" \
    --arg job_name "$CI_JOB_NAME" \
    --arg source_commit "$CI_COMMIT_SHA" \
    --arg project_path "$CI_PROJECT_PATH" \
    --arg server_url "$CI_SERVER_URL" \
    --arg project_url "$CI_PROJECT_URL" \
    --arg pipeline_url "$CI_PIPELINE_URL" \
    --arg job_api_url "${CI_API_V4_URL}/job" \
    --arg job_web_url "$expected_job_url" \
    --arg default_branch "$CI_DEFAULT_BRANCH" \
    --arg commit_branch "$CI_COMMIT_BRANCH" \
    --argjson commit_ref_protected true \
    '{schema_version:$schema,pipeline_source:$pipeline_source,pipeline_id:$pipeline_id,
      job_id:$job_id,project_id:$project_id,runner_id:$runner_id,job_name:$job_name,
      source_commit:$source_commit,project_path:$project_path,server_url:$server_url,
      project_url:$project_url,pipeline_url:$pipeline_url,default_branch:$default_branch,
      job_api_url:$job_api_url,job_web_url:$job_web_url,job_token_authenticated:true,
      commit_branch:$commit_branch,commit_ref_protected:$commit_ref_protected,scheduled:true}' \
    >"${trigger_file}.tmp"
  mv "${trigger_file}.tmp" "$trigger_file"
  export REDLINEDB_INTERACTION_TRIGGER_EVIDENCE_FILE="$trigger_file"
fi

command -v docker >/dev/null 2>&1 || {
  printf 'CI image lacks the Docker CLI\n' >&2
  exit 2
}
docker version --format '{{.Client.Version}} {{.Server.Version}}' >/dev/null
docker info --format '{{json .ID}}' | jq -e 'type == "string" and length > 0' >/dev/null

export REDLINEDB_CERT_ALLOW_IMAGE_PULL=1
exec bash ops/ci/interaction-volume-cert.sh "$mode"
