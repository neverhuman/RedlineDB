#!/usr/bin/env bash
# Exercise timeout and external-SIGTERM receipt preservation against the real smoke binary.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

export CI=true

set +e
REDLINEDB_CERT_INTERRUPTION_TEST_CASE=timeout \
  bash ops/ci/interaction-volume-cert.sh smoke
timeout_status=$?
set -e
case "$timeout_status" in
  124|137) ;;
  *)
    printf 'timeout receipt test returned %s, expected 124 or 137\n' "$timeout_status" >&2
    exit 1
    ;;
esac
timeout_progress='target/ci/interaction-volume/smoke-timeout/progress.json'
jq -e '
  .status == "timed_out" and
  .cause == "deadline_exceeded" and
  .lifecycle_phase == "deadline_exceeded" and
  .active_engine != null and
  .active_point != null and
  .deadline_unix_ms > 0 and
  .heartbeat_unix_ms >= .deadline_unix_ms
' "$timeout_progress" >/dev/null

REDLINEDB_CERT_INTERRUPTION_TEST_CASE=sigterm \
  bash ops/ci/interaction-volume-cert.sh smoke &
wrapper_pid=$!
sigterm_progress='target/ci/interaction-volume/smoke-sigterm/progress.json'
active=false
for _ in $(seq 1 120); do
  for live_progress in \
    target/ci/interaction-volume/runtime/redline-interaction-cert-smoke-sigterm-*/progress.json; do
    if [ -s "$live_progress" ] && \
      jq -e '.active_engine != null and .active_point != null' \
        "$live_progress" >/dev/null 2>&1; then
      active=true
      break 2
    fi
  done
  if ! kill -0 "$wrapper_pid" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
if [ "$active" != true ]; then
  kill -TERM "$wrapper_pid" >/dev/null 2>&1 || true
  wait "$wrapper_pid" >/dev/null 2>&1 || true
  printf 'SIGTERM receipt test never observed an active engine point\n' >&2
  exit 1
fi
kill -TERM "$wrapper_pid"
set +e
wait "$wrapper_pid"
sigterm_status=$?
set -e
[ "$sigterm_status" -eq 143 ] || {
  printf 'SIGTERM receipt test returned %s, expected 143\n' "$sigterm_status" >&2
  exit 1
}
jq -e '
  .status == "interrupted" and
  .cause == "external_sigterm" and
  .lifecycle_phase == "terminated" and
  .active_engine != null and
  .active_point != null and
  .deadline_unix_ms > .heartbeat_unix_ms
' "$sigterm_progress" >/dev/null

printf 'interaction-volume interruption receipts verified\n'
