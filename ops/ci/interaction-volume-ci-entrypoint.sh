#!/usr/bin/env bash
# Runtime CI boundary for the owned-Docker interaction-volume jobs.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

mode="${1:-}"
canonical_project_path='jeryu/redline-core'
canonical_branch='main'
trigger_contract='ops/ci/interaction-volume-trigger-contract.json'
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
  jq -e '
    .schema_version == "redline.interaction-volume-trigger/v4" and
    .authoritative_ci == "host-native-jeryu-required" and
    (.daily_enabled | not) and
    .permitted_daily_sources == [] and
    .attestation_authority.kind == "host_ci_jeryu_exact_head" and
    .attestation_authority.required_schema_version == "redline.host-ci-jeryu-attestation/v1" and
    .attestation_authority.canonical_https_origin == null and
    .attestation_authority.ca_bundle == null and
    .attestation_authority.ca_bundle_sha256 == null and
    .attestation_authority.signature_key_id == null and
    (.daily_blocker | type == "string" and length > 0)
  ' "$trigger_contract" >/dev/null || {
    printf 'daily authority contract is malformed or attempts an unauthenticated fallback\n' >&2
    exit 2
  }
  printf 'daily certification blocked: %s\n' \
    "$(jq -r '.daily_blocker' "$trigger_contract")" >&2
  exit 2
fi

command -v docker >/dev/null 2>&1 || {
  printf 'CI image lacks the Docker CLI\n' >&2
  exit 2
}
docker version --format '{{.Client.Version}} {{.Server.Version}}' >/dev/null
docker info --format '{{json .ID}}' | jq -e 'type == "string" and length > 0' >/dev/null

export REDLINEDB_CERT_ALLOW_IMAGE_PULL=1
exec bash ops/ci/interaction-volume-cert.sh "$mode"
