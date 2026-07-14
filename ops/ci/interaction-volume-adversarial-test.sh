#!/usr/bin/env bash
# Prove stale Docker evidence, endpoint substitution, and PostgreSQL cap overshoot fail closed.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

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
