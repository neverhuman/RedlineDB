#!/usr/bin/env bash
# Exercise timeout and external SIGTERM while PostgreSQL owns a live benchmark schema.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

export CI=true

wait_for_active_postgres() {
  wrapper_pid="${1:?wrapper pid required}"
  case_name="${2:?case name required}"
  for _ in $(seq 1 120); do
    for live_dir in \
      target/ci/interaction-volume/runtime/redline-interaction-cert-smoke-"${case_name}"-*; do
      progress="$live_dir/progress.json"
      evidence="$live_dir/execution-evidence.json"
      if [ -s "$progress" ] && [ -s "$evidence" ] && \
        jq -e '.active_engine == "postgres" and .active_point.engine == "postgres" and
          .lifecycle_phase != "run_start"' \
          "$progress" >/dev/null 2>&1; then
        container_id="$(jq -r '.postgres.container_id // empty' "$evidence")"
        schema_count="$(docker exec "$container_id" psql -U redline_cert -d redline_cert -Atqc \
          "SELECT COUNT(*) FROM pg_namespace WHERE nspname LIKE 'redline_interaction_%'" \
          2>/dev/null || true)"
        if [ "$schema_count" -gt 0 ] 2>/dev/null; then
          return 0
        fi
      fi
    done
    if ! kill -0 "$wrapper_pid" >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
  printf '%s interruption never observed PostgreSQL with a live benchmark schema\n' \
    "$case_name" >&2
  return 1
}

assert_cleanup() {
  receipt="${1:?receipt required}"
  jq -e '
    .schema_version == "redline.interaction-volume-cleanup/v1" and
    .child_stopped and
    .postgres_schema_cleanup_verified and
    .container_removed and
    .runtime_removed and
    (.container_id | length == 64)
  ' "$receipt/cleanup.json" >/dev/null
  container_id="$(jq -r '.container_id' "$receipt/cleanup.json")"
  if docker inspect "$container_id" >/dev/null 2>&1; then
    printf 'interruption cleanup left container %s\n' "$container_id" >&2
    exit 1
  fi
}

REDLINEDB_CERT_INTERRUPTION_TEST_CASE=timeout \
  bash ops/ci/interaction-volume-cert.sh smoke &
timeout_wrapper_pid=$!
wait_for_active_postgres "$timeout_wrapper_pid" timeout
set +e
wait "$timeout_wrapper_pid"
timeout_status=$?
set -e
case "$timeout_status" in
  124|137) ;;
  *)
    printf 'timeout receipt test returned %s, expected 124 or 137\n' "$timeout_status" >&2
    exit 1
    ;;
esac
timeout_receipt='target/ci/interaction-volume/smoke-timeout'
jq -e '
  .status == "timed_out" and
  .cause == "deadline_exceeded" and
  .lifecycle_phase == "deadline_exceeded" and
  .active_engine == "postgres" and
  .active_point.engine == "postgres" and
  .deadline_unix_ms > 0 and
  .heartbeat_unix_ms >= .deadline_unix_ms
' "$timeout_receipt/progress.json" >/dev/null
assert_cleanup "$timeout_receipt"

REDLINEDB_CERT_INTERRUPTION_TEST_CASE=sigterm \
  bash ops/ci/interaction-volume-cert.sh smoke &
sigterm_wrapper_pid=$!
wait_for_active_postgres "$sigterm_wrapper_pid" sigterm
kill -TERM "$sigterm_wrapper_pid"
set +e
wait "$sigterm_wrapper_pid"
sigterm_status=$?
set -e
[ "$sigterm_status" -eq 143 ] || {
  printf 'SIGTERM receipt test returned %s, expected 143\n' "$sigterm_status" >&2
  exit 1
}
sigterm_receipt='target/ci/interaction-volume/smoke-sigterm'
jq -e '
  .status == "interrupted" and
  .cause == "external_sigterm" and
  .lifecycle_phase == "terminated" and
  .active_engine == "postgres" and
  .active_point.engine == "postgres" and
  .deadline_unix_ms > .heartbeat_unix_ms
' "$sigterm_receipt/progress.json" >/dev/null
assert_cleanup "$sigterm_receipt"

if find target/ci/interaction-volume/runtime -mindepth 1 -maxdepth 1 -print -quit \
  | grep -q .; then
  printf 'interruption tests left runtime directories\n' >&2
  exit 1
fi
if docker ps -a --filter name=redline-interaction-cert --format '{{.Names}}' | grep -q .; then
  printf 'interruption tests left Docker containers\n' >&2
  exit 1
fi

printf 'PostgreSQL timeout/SIGTERM cleanup receipts verified\n'
