#!/usr/bin/env bash
# Digest-bound Redline/SQLite/PostgreSQL interaction-volume proof.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

if ! command -v rtk >/dev/null 2>&1; then
  rtk() {
    "$@"
  }
fi

mode="${1:-smoke}"
case "$mode" in
  smoke|daily) ;;
  *)
    printf 'usage: %s [smoke|daily]\n' "$0" >&2
    exit 2
    ;;
esac

interruption_case="${REDLINEDB_CERT_INTERRUPTION_TEST_CASE:-}"
if [ -n "$interruption_case" ]; then
  if [ "$mode" != smoke ] || [ "${CI:-}" != true ]; then
    printf 'interruption test cases are accepted only for smoke inside CI\n' >&2
    exit 2
  fi
  case "$interruption_case" in
    timeout|sigterm) ;;
    *)
      printf 'unsupported interruption test case: %s\n' "$interruption_case" >&2
      exit 2
      ;;
  esac
fi

command -v jq >/dev/null 2>&1 || {
  printf 'jq is required to verify certification receipts\n' >&2
  exit 2
}
command -v flock >/dev/null 2>&1 || {
  printf 'flock is required to serialize certification attempts\n' >&2
  exit 2
}
command -v findmnt >/dev/null 2>&1 || {
  printf 'findmnt is required to bind the three-engine storage contract\n' >&2
  exit 2
}

approved_profile='crates/bench/bench/interaction-volume-daily-v1.json'
approved_profile_sha256='165ffdffd7bec26f4ef7aa361a02f041a61fb62f8a3f58abc7e447ecf890cbb7'
actual_profile_sha256="$(sha256sum "$approved_profile" | awk '{print $1}')"
[ "$actual_profile_sha256" = "$approved_profile_sha256" ] || {
  printf 'approved daily profile digest mismatch: expected %s, got %s\n' \
    "$approved_profile_sha256" "$actual_profile_sha256" >&2
  exit 1
}
jq -e '
  .schema_version == "redline.interaction-volume-profile/v1" and
  .profile_id == "jain-session-event-daily-v1" and
  .config.seed == 7 and
  .config.max_data_bytes == 2147483648 and
  .config.max_idle_growth_bytes == 16777216
' "$approved_profile" >/dev/null

trigger_contract='ops/ci/interaction-volume-trigger-contract.json'
jq -e --arg profile_sha "$approved_profile_sha256" '
  .schema_version == "redline.interaction-volume-trigger/v1" and
  .contract_id == "interaction-volume-daily-v1" and
  .authoritative_ci == ".gitlab-ci.yml" and
  .daily_job == "interaction-volume-daily" and
  .required_smoke_job == "interaction-volume-smoke" and
  .resource_group == "redline-heavy-benchmark" and
  .retry == 0 and
  (.interruptible | not) and
  .canonical_profile_sha256 == $profile_sha and
  .artifact_retention_days == 90 and
  .interruption_receipt_tests == ["timeout", "sigterm"]
' "$trigger_contract" >/dev/null
if [ "$mode" = daily ] && [ "${CI:-}" = true ]; then
  [ "${REDLINEDB_INTERACTION_TRIGGER_CONTRACT:-}" = interaction-volume-daily-v1 ] || {
    printf 'CI daily run is missing the checked-in trigger contract binding\n' >&2
    exit 2
  }
fi

postgres_digest='sha256:786dab398303b8ce7cb76b407bb21ef2e4dfbbbd4c6abcf3d29b3130467ffdbc'
postgres_image="${REDLINEDB_POSTGRES_CERT_IMAGE:-docker.io/library/postgres@${postgres_digest}}"
postgres_backend='docker'
if [ "${REDLINEDB_POSTGRES_CERT_CI_SERVICE:-0}" = 1 ]; then
  postgres_backend='ci-service'
fi
postgres_digest_verified=false
postgres_digest_observation='unverified'
postgres_endpoint_scope='ci_service'
storage_class='unmatched_ci_service'
storage_same_mount=false
storage_durable=false
postgres_bind_mode='service_managed'
container_name="redline-interaction-cert-${mode}-$$"
receipt_name="$mode"
if [ -n "$interruption_case" ]; then
  receipt_name="${mode}-${interruption_case}"
fi
receipt_dir="target/ci/interaction-volume/${receipt_name}"
mkdir -p "$(dirname "$receipt_dir")"
exec 9>"$(dirname "$receipt_dir")/.${receipt_name}.lock"
flock -n 9 || {
  printf 'another %s interaction-volume certificate is already running\n' "$receipt_name" >&2
  exit 1
}
scratch_parent="${REDLINEDB_INTERACTION_CERT_TMPDIR:-${repo_root}/target/ci/interaction-volume/runtime}"
mkdir -p "$scratch_parent"
scratch_parent="$(realpath "$scratch_parent")"
[ -w "$scratch_parent" ] || {
  printf 'interaction-volume runtime root is not writable: %s\n' "$scratch_parent" >&2
  exit 2
}
scratch_dir="${scratch_parent%/}/redline-interaction-cert-${receipt_name}-$$"
container_id=''
cert_pid=''

# shellcheck disable=SC2317 # Invoked by traps below.
preserve_receipts() {
  mkdir -p "$receipt_dir"
  for receipt in execution-evidence.json attempt.json progress.json raw-runs.json manifest.json; do
    if [ -s "$scratch_dir/$receipt" ]; then
      cp "$scratch_dir/$receipt" "$receipt_dir/.${receipt}.tmp"
      mv "$receipt_dir/.${receipt}.tmp" "$receipt_dir/$receipt"
    fi
  done
}

# shellcheck disable=SC2317 # Invoked by the EXIT trap below.
cleanup() {
  if [ -n "$cert_pid" ]; then
    kill -TERM "$cert_pid" >/dev/null 2>&1 || true
    wait "$cert_pid" >/dev/null 2>&1 || true
  fi
  if [ -n "$container_id" ]; then
    docker exec -u 0 "$container_id" chown -R "$(id -u):$(id -g)" \
      /var/lib/postgresql/data >/dev/null 2>&1 || true
    docker rm -f "$container_id" >/dev/null 2>&1 || true
  fi
  rm -rf "$scratch_dir"
}

# shellcheck disable=SC2317 # Invoked by the INT/TERM traps below.
handle_signal() {
  signal_status="$1"
  signal_cause="$2"
  if [ -n "$cert_pid" ]; then
    kill -TERM "$cert_pid" >/dev/null 2>&1 || true
    wait "$cert_pid" >/dev/null 2>&1 || true
    cert_pid=''
  fi
  annotate_progress "$signal_cause" interrupted terminated
  preserve_receipts || true
  exit "$signal_status"
}

annotate_progress() {
  cause="$1"
  status="$2"
  phase="$3"
  progress="$scratch_dir/progress.json"
  [ -s "$progress" ] || return 0
  heartbeat_ms="$(date +%s%3N)"
  jq --arg cause "$cause" --arg status "$status" --arg phase "$phase" \
    --argjson heartbeat "$heartbeat_ms" \
    '.status=$status | .cause=$cause | .lifecycle_phase=$phase | .heartbeat_unix_ms=$heartbeat' \
    "$progress" >"${progress}.tmp"
  mv "${progress}.tmp" "$progress"
}

trap cleanup EXIT
trap 'handle_signal 130 external_sigint' INT
trap 'handle_signal 143 external_sigterm' TERM

rm -rf "$receipt_dir" "$scratch_dir"
mkdir -p "$receipt_dir" "$scratch_dir/dbs"
postgres_data_root="$scratch_dir/postgres-data"

if [ "$postgres_backend" = ci-service ]; then
  [ "${CI:-}" = true ] || {
    printf 'CI PostgreSQL service mode is accepted only when CI=true\n' >&2
    exit 2
  }
  [ "${REDLINEDB_POSTGRES_CERT_SERVICE_DIGEST:-}" = "$postgres_digest" ] || {
    printf 'CI PostgreSQL service digest is absent or differs from %s\n' "$postgres_digest" >&2
    exit 1
  }
  [ -n "${REDLINEDB_BENCH_POSTGRES_URL:-}" ] || {
    printf 'CI PostgreSQL service mode requires REDLINEDB_BENCH_POSTGRES_URL\n' >&2
    exit 2
  }
  export REDLINEDB_BENCH_POSTGRES_ISOLATED=1
  export REDLINEDB_BENCH_POSTGRES_CI_SERVICE=1
  postgres_data_root='ci-service:postgres-cert'
  postgres_mount_identity='unobservable-ci-service-mount'
else
  command -v docker >/dev/null 2>&1 || {
    printf 'docker is required for the isolated local PostgreSQL reference\n' >&2
    exit 2
  }
  if ! docker image inspect "$postgres_image" >/dev/null 2>&1; then
    if [ "${REDLINEDB_CERT_ALLOW_IMAGE_PULL:-0}" != 1 ]; then
      printf 'pinned PostgreSQL image is not cached; preload %s or set REDLINEDB_CERT_ALLOW_IMAGE_PULL=1\n' \
        "$postgres_image" >&2
      exit 2
    fi
    docker pull "$postgres_image" >/dev/null
  fi
  image_repo_digests="$(docker image inspect --format '{{join .RepoDigests " "}}' "$postgres_image")"
  case " $image_repo_digests " in
    *"@${postgres_digest}"*) ;;
    *)
      printf 'cached PostgreSQL image is not bound to %s: %s\n' \
        "$postgres_digest" "$image_repo_digests" >&2
      exit 1
      ;;
  esac
  postgres_digest_verified=true
  postgres_digest_observation='docker_image_inspect_repo_digest'
  postgres_endpoint_scope='loopback'
  storage_class='shared_host_durable_bind'
  storage_same_mount=true
  postgres_bind_mode='rw'
  mkdir -p "$postgres_data_root"
  container_id="$(
    docker run --rm -d \
      --name "$container_name" \
      --memory 3g \
      --pids-limit 512 \
      --mount "type=bind,src=${postgres_data_root},dst=/var/lib/postgresql/data" \
      --tmpfs /var/run/postgresql:rw,nosuid,nodev,size=67108864 \
      --tmpfs /tmp:rw,nosuid,nodev,size=67108864 \
      -e POSTGRES_DB=redline_cert \
      -e POSTGRES_USER=redline_cert \
      -e POSTGRES_PASSWORD=redline-cert-local-only \
      -e POSTGRES_INITDB_ARGS=--data-checksums \
      -p 127.0.0.1::5432 \
      --health-cmd 'pg_isready -U redline_cert -d redline_cert' \
      --health-interval 1s \
      --health-timeout 3s \
      --health-retries 60 \
      --health-start-period 2s \
      "$postgres_image"
  )"
fi

local_device="$(stat -c %d "$scratch_dir/dbs")"
local_fstype="$(stat -f -c %T "$scratch_dir/dbs")"
local_mount_contract="$(findmnt -T "$scratch_dir/dbs" -n -o MAJ:MIN,FSTYPE,SOURCE,OPTIONS)"
local_mount_identity="device=${local_device};fstype=${local_fstype};mount=${local_mount_contract}"
if [ "$postgres_backend" = docker ]; then
  postgres_device="$(stat -c %d "$postgres_data_root")"
  postgres_fstype="$(stat -f -c %T "$postgres_data_root")"
  postgres_mount_contract="$(findmnt -T "$postgres_data_root" -n -o MAJ:MIN,FSTYPE,SOURCE,OPTIONS)"
  postgres_mount_identity="device=${postgres_device};fstype=${postgres_fstype};mount=${postgres_mount_contract}"
  if [ "$local_mount_identity" != "$postgres_mount_identity" ]; then
    storage_same_mount=false
    storage_class='mismatched_host_mounts'
  fi
  case "$local_fstype" in
    tmpfs|ramfs)
      storage_durable=false
      storage_class='shared_nondurable_memory_mount'
      ;;
    *) storage_durable=true ;;
  esac
fi

if [ "$postgres_backend" = docker ]; then
  for _ in $(seq 1 90); do
    health="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$container_id")"
    if [ "$health" = healthy ]; then
      break
    fi
    if [ "$health" = unhealthy ]; then
      docker logs "$container_id" >&2 || true
      printf 'isolated PostgreSQL became unhealthy\n' >&2
      exit 1
    fi
    sleep 1
  done
  health="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$container_id")"
  [ "$health" = healthy ] || {
    docker logs "$container_id" >&2 || true
    printf 'timed out waiting for isolated PostgreSQL\n' >&2
    exit 1
  }

  postgres_port="$(docker inspect -f '{{(index (index .NetworkSettings.Ports "5432/tcp") 0).HostPort}}' "$container_id")"
  case "$postgres_port" in
    ''|*[!0-9]*)
      printf 'failed to resolve isolated PostgreSQL port\n' >&2
      exit 1
      ;;
  esac

  export REDLINEDB_BENCH_POSTGRES_ISOLATED=1
  export REDLINEDB_BENCH_POSTGRES_URL="host=127.0.0.1 port=${postgres_port} user=redline_cert password=redline-cert-local-only dbname=redline_cert connect_timeout=5"
fi

if [ "$mode" = daily ]; then
  rtk cargo build --release --locked -p redlinedb-bench --bin interaction_volume_cert
  cert_bin='target/release/interaction_volume_cert'
  cert_timeout=7200
  cert_max_file_bytes=2147483648
  cert_args=(
    --mode release
    --out-dir "$scratch_dir"
    --approved-profile "$approved_profile"
  )
else
  rtk cargo build --locked -p redlinedb-bench --bin interaction_volume_cert
  cert_bin='target/debug/interaction_volume_cert'
  cert_timeout=300
  cert_max_file_bytes=67108864
  cert_args=(
    --mode smoke
    --out-dir "$scratch_dir"
    --threads "1,2"
    --operations-per-thread 50
    --repetitions 1
    --sessions 8
    --payload-bytes 64
    --warmup-operations-per-thread 8
    --idle-observation-secs 1
    --soak-observation-secs 2
    --max-data-bytes 67108864
    --max-idle-growth-bytes 1048576
  )
fi

if [ "$interruption_case" = timeout ]; then
  cert_timeout=8
fi

source_commit="$(git rev-parse --verify HEAD)"
case "$source_commit" in
  *[!0-9a-f]*|'')
    printf 'could not derive a full source commit from the checked-out repository\n' >&2
    exit 1
    ;;
esac
[ "${#source_commit}" -eq 40 ] || {
  printf 'source commit must be a full 40-character SHA: %s\n' "$source_commit" >&2
  exit 1
}
if [ -n "${CI_COMMIT_SHA:-}" ] && [ "$CI_COMMIT_SHA" != "$source_commit" ]; then
  printf 'CI commit %s differs from checked-out source %s\n' "$CI_COMMIT_SHA" "$source_commit" >&2
  exit 1
fi
source_dirty=false
if [ -n "$(git status --porcelain --untracked-files=normal)" ]; then
  source_dirty=true
fi
binary_sha256="$(sha256sum "$cert_bin" | awk '{print $1}')"
evidence_file="$scratch_dir/execution-evidence.json"
evidence_backend='ci_service'
if [ "$postgres_backend" = docker ]; then
  evidence_backend='docker_bind'
fi
jq -n \
  --arg schema 'redline.interaction-volume-execution-evidence/v1' \
  --arg generator 'ops/ci/interaction-volume-cert.sh' \
  --arg source_commit "$source_commit" \
  --argjson source_dirty "$source_dirty" \
  --arg binary_sha256 "$binary_sha256" \
  --arg image_digest "$postgres_digest" \
  --arg digest_observation "$postgres_digest_observation" \
  --argjson digest_verified "$postgres_digest_verified" \
  --arg backend "$evidence_backend" \
  --arg endpoint_scope "$postgres_endpoint_scope" \
  --arg storage_class "$storage_class" \
  --arg local_root "$scratch_dir/dbs" \
  --arg postgres_root "$postgres_data_root" \
  --arg local_mount "$local_mount_identity" \
  --arg postgres_mount "$postgres_mount_identity" \
  --argjson same_mount "$storage_same_mount" \
  --argjson durable "$storage_durable" \
  --arg bind_mode "$postgres_bind_mode" \
  '{
    schema_version:$schema,
    generator:$generator,
    source_commit:$source_commit,
    source_dirty:$source_dirty,
    binary_sha256:$binary_sha256,
    postgres:{
      image_digest:$image_digest,
      digest_observation:$digest_observation,
      digest_verified:$digest_verified,
      backend:$backend,
      isolation_verified:true,
      dedicated_instance:true,
      endpoint_scope:$endpoint_scope
    },
    storage:{
      class:$storage_class,
      local_database_root:$local_root,
      postgres_data_root:$postgres_root,
      local_mount_identity:$local_mount,
      postgres_mount_identity:$postgres_mount,
      same_mount:$same_mount,
      durable:$durable,
      postgres_bind_mode:$bind_mode
    }
  }' >"${evidence_file}.tmp"
mv "${evidence_file}.tmp" "$evidence_file"
deadline_unix_ms="$(( $(date +%s%3N) + cert_timeout * 1000 ))"
cert_args+=(
  --execution-evidence "$evidence_file"
  --deadline-secs "$cert_timeout"
  --deadline-unix-ms "$deadline_unix_ms"
)

set +e
(
  # bash reports RLIMIT_FSIZE in 1024-byte blocks. This kernel-enforced ceiling
  # remains active even if an engine's own storage snapshot path wedges.
  ulimit -f "$((cert_max_file_bytes / 1024))"
  exec timeout --signal=TERM --kill-after=30 "$cert_timeout" "$cert_bin" "${cert_args[@]}"
) &
cert_pid=$!
wait "$cert_pid"
cert_status=$?
cert_pid=''
set -e

case "$cert_status" in
  124|137)
    annotate_progress deadline_exceeded timed_out deadline_exceeded
    ;;
esac

preserve_receipts
for receipt in attempt.json progress.json; do
  [ -s "$receipt_dir/$receipt" ] || {
    printf 'certification did not produce pre-run/progress receipt %s\n' "$receipt" >&2
    exit 1
  }
done
jq -e --arg postgres_digest "$postgres_digest" \
  --arg evidence_sha "$(sha256sum "$receipt_dir/execution-evidence.json" | awk '{print $1}')" '
  .status == "in_progress" and
  (.planned_runs | length) > 0 and
  .postgres_image_digest == $postgres_digest and
  .execution_evidence_sha256 == $evidence_sha and
  .artifact_sha256 == .execution_evidence.binary_sha256
' \
  "$receipt_dir/attempt.json" >/dev/null
jq -e '
  .planned_runs > 0 and
  (.lifecycle_phase | type == "string" and length > 0) and
  (.heartbeat_unix_ms | type == "number") and
  (.deadline_unix_ms | type == "number" and . > 0) and
  has("active_engine") and has("active_point") and has("cause")
' \
  "$receipt_dir/progress.json" >/dev/null

if [ ! -s "$receipt_dir/raw-runs.json" ] || [ ! -s "$receipt_dir/manifest.json" ]; then
  printf 'certification ended before final receipts (status %s); atomic attempt/progress receipts were preserved\n' \
    "$cert_status" >&2
  [ "$cert_status" -ne 0 ] && exit "$cert_status"
  exit 1
fi

raw_sha="$(sha256sum "$receipt_dir/raw-runs.json" | awk '{print $1}')"
jq -e --arg raw_sha "$raw_sha" '.raw_receipt_sha256 == $raw_sha' \
  "$receipt_dir/manifest.json" >/dev/null
binary_sha="$(sha256sum "$cert_bin" | awk '{print $1}')"
jq -e --arg binary_sha "$binary_sha" '.artifact_sha256 == $binary_sha' \
  "$receipt_dir/manifest.json" >/dev/null
jq -e --arg postgres_digest "$postgres_digest" \
  '.postgres_image_digest == $postgres_digest' \
  "$receipt_dir/manifest.json" >/dev/null
jq -e --arg evidence_sha "$(sha256sum "$receipt_dir/execution-evidence.json" | awk '{print $1}')" \
  '.execution_evidence_sha256 == $evidence_sha and
   .artifact_sha256 == .execution_evidence.binary_sha256 and
   .storage_contract == .execution_evidence.storage' \
  "$receipt_dir/manifest.json" >/dev/null
jq -e --arg attempt_sha "$(sha256sum "$receipt_dir/attempt.json" | awk '{print $1}')" \
  '.attempt_receipt == "attempt.json" and .attempt_receipt_sha256 == $attempt_sha' \
  "$receipt_dir/manifest.json" >/dev/null
jq -e '. as $root | (($root.runs | length) > 0) and all($root.runs[]; .plan_integrity_verified and .reopen_verified)' \
  "$receipt_dir/raw-runs.json" >/dev/null
jq -e 'all(.runs[];
  (.storage_semantics.cross_engine_comparable | not) and
  .storage_semantics.sampled_inside_timed_window and
  .storage_semantics.continuously_sampled and
  (.workload_storage_samples | length) >= 2 and
  (.lifecycle_storage_samples | length) >= 8
)' "$receipt_dir/raw-runs.json" >/dev/null

if [ "$postgres_backend" = docker ]; then
  remaining_schemas="$(
    docker exec "$container_id" psql -U redline_cert -d redline_cert -Atqc \
      "SELECT COUNT(*) FROM pg_namespace WHERE nspname LIKE 'redline_interaction_%'"
  )"
  [ "$remaining_schemas" = 0 ] || {
    printf 'PostgreSQL benchmark schemas leaked: %s\n' "$remaining_schemas" >&2
    exit 1
  }
fi
if find "$scratch_dir/dbs" -type f -print -quit 2>/dev/null | grep -q .; then
  printf 'local benchmark database files were not cleaned\n' >&2
  exit 1
fi

if [ "$mode" = smoke ]; then
  jq -e '.status == "informational_pass" and .mechanics_passed and (.release_eligible | not)' \
    "$receipt_dir/manifest.json" >/dev/null
else
  jq -e --arg profile_sha "$approved_profile_sha256" '
    .status == "pass" and
    .mechanics_passed and
    .release_eligible and
    .canonical_profile and
    .config.seed == 7 and
    .approved_profile_sha256 == $profile_sha and
    .reference_cleanup_verified and
    .provenance_bound and
    .storage_comparison_eligible and
    .storage_contract.class == "shared_host_durable_bind" and
    .bounded_reference_win_eligible
  ' \
    "$receipt_dir/manifest.json" >/dev/null
fi

exit "$cert_status"
