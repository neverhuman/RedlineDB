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
adversarial_case="${REDLINEDB_CERT_ADVERSARIAL_TEST_CASE:-}"
if [ -n "$adversarial_case" ]; then
  if [ "$mode" != smoke ] || [ "${CI:-}" != true ]; then
    printf 'adversarial test cases are accepted only for smoke inside CI\n' >&2
    exit 2
  fi
  case "$adversarial_case" in
    stale_container|endpoint_mismatch|postgres_storage_overshoot) ;;
    *)
      printf 'unsupported adversarial test case: %s\n' "$adversarial_case" >&2
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
command -v fallocate >/dev/null 2>&1 || {
  printf 'fallocate is required for the emergency storage reserve\n' >&2
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
  .schema_version == "redline.interaction-volume-trigger/v4" and
  .contract_id == "interaction-volume-daily-v1" and
  .authoritative_ci == "host-native-jeryu-required" and
  (.daily_enabled | not) and
  (.daily_blocker | type == "string" and length > 0) and
  .daily_job == "interaction-volume-daily" and
  .required_smoke_job == "interaction-volume-smoke" and
  .canonical_project_path == "jeryu/redline-core" and
  .canonical_branch == "main" and
  .permitted_daily_sources == [] and
  .resource_group == "redline-heavy-benchmark" and
  .retry == 0 and
  (.interruptible | not) and
  .canonical_profile_sha256 == $profile_sha and
  .ci_entrypoint == "ops/ci/interaction-volume-ci-entrypoint.sh" and
  .attestation_authority.kind == "host_ci_jeryu_exact_head" and
  .attestation_authority.required_schema_version == "redline.host-ci-jeryu-attestation/v1" and
  .attestation_authority.canonical_https_origin == null and
  .attestation_authority.ca_bundle == null and
  .attestation_authority.ca_bundle_sha256 == null and
  .attestation_authority.signature_key_id == null and
  .docker_service_digest == "sha256:aa3df78ecf320f5fafdce71c659f1629e96e9de0968305fe1de670e0ca9176ce" and
  .runtime_trigger_receipt == "trigger-evidence.json" and
  .artifact_retention_days == 90 and
  .interruption_receipt_tests == ["postgres_timeout", "postgres_sigterm"] and
  .adversarial_tests == ["fake_ci_responder", "stale_container", "endpoint_mismatch", "postgres_storage_overshoot"]
' "$trigger_contract" >/dev/null
if [ "$mode" = daily ]; then
  printf 'daily certification blocked: %s\n' \
    "$(jq -r '.daily_blocker' "$trigger_contract")" >&2
  exit 2
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
postgres_rootfs_read_only=false
postgres_log_driver='unobservable'
container_name="redline-interaction-cert-${mode}-$$"
container_run_id="$(printf '%s' "${container_name}-$(date +%s%N)-${RANDOM}" | sha256sum | awk '{print $1}')"
receipt_name="$mode"
if [ -n "$interruption_case" ]; then
  receipt_name="${mode}-${interruption_case}"
elif [ -n "$adversarial_case" ]; then
  receipt_name="${mode}-adversarial-${adversarial_case}"
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
docker_daemon_id=''
postgres_endpoint_host=''
postgres_port=''
reserve_path=''
reserve_bytes=0
ci_trigger_json=null

# shellcheck disable=SC2317 # Invoked by traps below.
preserve_receipts() {
  mkdir -p "$receipt_dir"
  for receipt in execution-evidence.json attempt.json progress.json raw-runs.json manifest.json trigger-evidence.json cert.stderr.log; do
    if [ -s "$scratch_dir/$receipt" ]; then
      cp "$scratch_dir/$receipt" "$receipt_dir/.${receipt}.tmp"
      mv "$receipt_dir/.${receipt}.tmp" "$receipt_dir/$receipt"
    fi
  done
}

# shellcheck disable=SC2317 # Invoked by the EXIT trap below.
cleanup() {
  original_status=$?
  trap - EXIT
  set +e
  child_stopped=true
  schema_cleanup_verified=true
  container_removed=true
  runtime_removed=true
  if [ -n "$cert_pid" ]; then
    kill -TERM "$cert_pid" >/dev/null 2>&1 || true
    wait "$cert_pid" >/dev/null 2>&1 || true
    if kill -0 "$cert_pid" >/dev/null 2>&1; then
      child_stopped=false
    fi
  fi
  if [ -n "$container_id" ]; then
    if docker inspect "$container_id" >/dev/null 2>&1; then
      if ! schemas="$(docker exec "$container_id" psql -U redline_cert -d redline_cert -Atqc \
        "SELECT nspname FROM pg_namespace WHERE nspname LIKE 'redline_interaction_%'" 2>/dev/null)"; then
        schema_cleanup_verified=false
      else
        while IFS= read -r schema; do
          [ -n "$schema" ] || continue
          if ! grep -Eq '^redline_interaction_[0-9a-f]{20}$' <<<"$schema"; then
            schema_cleanup_verified=false
            continue
          fi
          docker exec "$container_id" psql -v ON_ERROR_STOP=1 -U redline_cert -d redline_cert \
            -c "DROP SCHEMA IF EXISTS \"${schema}\" CASCADE" >/dev/null 2>&1 || \
            schema_cleanup_verified=false
        done <<<"$schemas"
        remaining_schemas="$(docker exec "$container_id" psql -U redline_cert -d redline_cert -Atqc \
          "SELECT COUNT(*) FROM pg_namespace WHERE nspname LIKE 'redline_interaction_%'" 2>/dev/null)"
        [ "$remaining_schemas" = 0 ] || schema_cleanup_verified=false
      fi
    fi
    docker exec -u 0 "$container_id" chown -R "$(id -u):$(id -g)" \
      /var/lib/postgresql/data >/dev/null 2>&1 || true
    docker rm -f "$container_id" >/dev/null 2>&1 || container_removed=false
    if docker inspect "$container_id" >/dev/null 2>&1; then
      container_removed=false
    fi
  fi
  rm -rf "$scratch_dir"
  [ ! -e "$scratch_dir" ] || runtime_removed=false
  mkdir -p "$receipt_dir"
  jq -n \
    --arg schema 'redline.interaction-volume-cleanup/v1' \
    --arg container_id "$container_id" \
    --argjson child_stopped "$child_stopped" \
    --argjson schema_cleanup_verified "$schema_cleanup_verified" \
    --argjson container_removed "$container_removed" \
    --argjson runtime_removed "$runtime_removed" \
    '{schema_version:$schema,child_stopped:$child_stopped,
      postgres_schema_cleanup_verified:$schema_cleanup_verified,
      container_id:$container_id,container_removed:$container_removed,
      runtime_removed:$runtime_removed}' >"$receipt_dir/.cleanup.json.tmp"
  mv "$receipt_dir/.cleanup.json.tmp" "$receipt_dir/cleanup.json"
  manifest_cleanup_bound=true
  cleanup_sha="$(sha256sum "$receipt_dir/cleanup.json" | awk '{print $1}')"
  if [ -s "$receipt_dir/manifest.json" ]; then
    if jq --arg cleanup_sha "$cleanup_sha" \
      '.cleanup_receipt = "cleanup.json" | .cleanup_receipt_sha256 = $cleanup_sha' \
      "$receipt_dir/manifest.json" >"$receipt_dir/.manifest.json.tmp" && \
      mv "$receipt_dir/.manifest.json.tmp" "$receipt_dir/manifest.json" && \
      jq -e --arg cleanup_sha "$cleanup_sha" '
        .schema_version == "redline.interaction-volume-cert/v4" and
        .cleanup_receipt == "cleanup.json" and
        .cleanup_receipt_sha256 == $cleanup_sha
      ' "$receipt_dir/manifest.json" >/dev/null
    then
      :
    else
      manifest_cleanup_bound=false
      rm -f "$receipt_dir/.manifest.json.tmp"
    fi
  elif [ -z "$adversarial_case" ] && [ -z "$interruption_case" ]; then
    manifest_cleanup_bound=false
  fi
  if [ "$original_status" -eq 0 ] && \
    { [ "$child_stopped" != true ] || [ "$schema_cleanup_verified" != true ] || \
      [ "$container_removed" != true ] || [ "$runtime_removed" != true ] || \
      [ "$manifest_cleanup_bound" != true ]; }; then
    original_status=1
  fi
  exit "$original_status"
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
if [ "$mode" = daily ]; then
  cert_hard_storage_bytes=2147483648
  reserve_bytes=536870912
  cert_stop_storage_bytes=1610612736
else
  cert_hard_storage_bytes=134217728
  reserve_bytes=33554432
  cert_stop_storage_bytes=100663296
fi
available_bytes="$(df --output=avail -B1 "$scratch_dir" | awk 'NR==2 {print $1}')"
case "$available_bytes" in
  ''|*[!0-9]*)
    printf 'could not observe free bytes for certification storage\n' >&2
    exit 1
    ;;
esac
required_free_bytes="$((cert_hard_storage_bytes + reserve_bytes))"
[ "$available_bytes" -gt "$required_free_bytes" ] || {
  printf 'certification storage has %s free bytes; requires more than %s for cap plus reserve\n' \
    "$available_bytes" "$required_free_bytes" >&2
  exit 1
}
reserve_path="$scratch_dir/.emergency-storage-reserve"
fallocate -l "$reserve_bytes" "$reserve_path"
reserve_allocated_bytes="$(( $(stat -c %b "$reserve_path") * 512 ))"
[ "$reserve_allocated_bytes" -ge "$reserve_bytes" ] || {
  printf 'emergency reserve allocated %s bytes, expected %s\n' \
    "$reserve_allocated_bytes" "$reserve_bytes" >&2
  exit 1
}

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
  postgres_endpoint_host='postgres-cert'
  postgres_port=5432
  postgres_data_root='ci-service:postgres-cert'
  postgres_mount_identity='unobservable-ci-service-mount'
else
  command -v docker >/dev/null 2>&1 || {
    printf 'docker is required for the isolated local PostgreSQL reference\n' >&2
    exit 2
  }
  docker info >/dev/null 2>&1 || {
    printf 'Docker CLI cannot reach the certification daemon (%s)\n' "${DOCKER_HOST:-local socket}" >&2
    exit 2
  }
  docker_daemon_id="$(docker info --format '{{json .ID}}' | jq -er '.')"
  [ -n "$docker_daemon_id" ] || {
    printf 'Docker daemon returned an empty identity\n' >&2
    exit 1
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
  postgres_rootfs_read_only=true
  postgres_log_driver='none'
  mkdir -p "$postgres_data_root"
  postgres_publish_ip='127.0.0.1'
  postgres_endpoint_host='127.0.0.1'
  if [ -n "${DOCKER_HOST:-}" ]; then
    case "$DOCKER_HOST" in
      tcp://docker:*)
        [ "${CI:-}" = true ] || {
          printf 'Docker daemon service endpoint is accepted only inside CI\n' >&2
          exit 2
        }
        postgres_publish_ip='0.0.0.0'
        postgres_endpoint_host='docker'
        postgres_endpoint_scope='docker_daemon_service'
        export REDLINEDB_BENCH_POSTGRES_DOCKER_DAEMON_SERVICE=1
        ;;
      unix://*|npipe://*) ;;
      *)
        printf 'unsupported Docker daemon endpoint for certification: %s\n' "$DOCKER_HOST" >&2
        exit 2
        ;;
    esac
  fi
  bind_sentinel="$postgres_data_root/.redline-bind-sentinel"
  printf '%s\n' "$container_run_id" >"$bind_sentinel"
  docker run --rm --entrypoint sh \
    --mount "type=bind,src=${postgres_data_root},dst=/var/lib/postgresql/data" \
    "$postgres_image" -c \
    'test "$(cat /var/lib/postgresql/data/.redline-bind-sentinel)" = "$1"' \
    redline-cert "$container_run_id" || {
      printf 'Docker daemon cannot observe the job storage bind path\n' >&2
      exit 1
    }
  rm -f "$bind_sentinel"
  container_id="$(
    docker run --rm -d \
      --name "$container_name" \
      --label "redline.interaction-volume.run_id=${container_run_id}" \
      --memory 3g \
      --pids-limit 512 \
      --read-only \
      --log-driver none \
      --mount "type=bind,src=${postgres_data_root},dst=/var/lib/postgresql/data" \
      --tmpfs /var/run/postgresql:rw,nosuid,nodev,size=67108864 \
      --tmpfs /tmp:rw,nosuid,nodev,size=67108864 \
      -e POSTGRES_DB=redline_cert \
      -e POSTGRES_USER=redline_cert \
      -e POSTGRES_PASSWORD=redline-cert-local-only \
      -e POSTGRES_INITDB_ARGS=--data-checksums \
      -p "${postgres_publish_ip}::5432" \
      --health-cmd 'pg_isready -U redline_cert -d redline_cert' \
      --health-interval 1s \
      --health-timeout 3s \
      --health-retries 60 \
      --health-start-period 2s \
      "$postgres_image"
  )"
  case "$container_id" in
    *[!0-9a-f]*|'')
      printf 'Docker returned an invalid certification container id: %s\n' "$container_id" >&2
      exit 1
      ;;
  esac
  [ "${#container_id}" -eq 64 ] || {
    printf 'Docker container id must be a full 64-character identity\n' >&2
    exit 1
  }
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
  export REDLINEDB_BENCH_POSTGRES_CONTAINER_ID="$container_id"
  export REDLINEDB_BENCH_POSTGRES_URL="host=${postgres_endpoint_host} port=${postgres_port} user=redline_cert password=redline-cert-local-only dbname=redline_cert connect_timeout=5"
fi

if [ "$mode" = daily ]; then
  rtk cargo build --release --locked -p redlinedb-bench --bin interaction_volume_cert 9>&-
  cert_bin='target/release/interaction_volume_cert'
  cert_timeout=7200
  cert_max_file_bytes="$cert_stop_storage_bytes"
  cert_args=(
    --mode release
    --out-dir "$scratch_dir"
    --approved-profile "$approved_profile"
  )
else
  rtk cargo build --locked -p redlinedb-bench --bin interaction_volume_cert 9>&-
  cert_bin='target/debug/interaction_volume_cert'
  cert_timeout=300
  cert_max_file_bytes="$cert_stop_storage_bytes"
  smoke_threads='1,2'
  smoke_operations=50
  smoke_seed=7
  smoke_max_data_bytes="$cert_hard_storage_bytes"
  if [ -n "$interruption_case" ]; then
    smoke_threads=1
    smoke_operations=20000
    smoke_seed=8
  elif [ "$adversarial_case" = postgres_storage_overshoot ]; then
    smoke_threads=1
    smoke_operations=50
    smoke_seed=8
    smoke_max_data_bytes=33554432
  fi
  cert_args=(
    --mode smoke
    --out-dir "$scratch_dir"
    --threads "$smoke_threads"
    --operations-per-thread "$smoke_operations"
    --repetitions 1
    --sessions 8
    --payload-bytes 64
    --warmup-operations-per-thread 8
    --seed "$smoke_seed"
    --idle-observation-secs 1
    --soak-observation-secs 2
    --max-data-bytes "$smoke_max_data_bytes"
    --max-idle-growth-bytes 1048576
  )
fi

if [ "$interruption_case" = timeout ]; then
  # Leave enough room for live Docker/provenance observation and PostgreSQL schema setup on a
  # loaded CI runner. The interruption workload is deliberately much longer than this deadline.
  cert_timeout=30
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
  --arg schema 'redline.interaction-volume-execution-evidence/v3' \
  --arg generator 'ops/ci/interaction-volume-cert.sh' \
  --arg source_commit "$source_commit" \
  --argjson source_dirty "$source_dirty" \
  --arg binary_sha256 "$binary_sha256" \
  --arg image_digest "$postgres_digest" \
  --arg digest_observation "$postgres_digest_observation" \
  --argjson digest_verified "$postgres_digest_verified" \
  --arg backend "$evidence_backend" \
  --arg endpoint_scope "$postgres_endpoint_scope" \
  --arg container_id "$container_id" \
  --arg container_name "$container_name" \
  --arg container_run_id "$container_run_id" \
  --arg docker_daemon_id "$docker_daemon_id" \
  --arg endpoint_host "$postgres_endpoint_host" \
  --argjson endpoint_port "$postgres_port" \
  --argjson rootfs_read_only "$postgres_rootfs_read_only" \
  --arg log_driver "$postgres_log_driver" \
  --arg storage_class "$storage_class" \
  --arg local_root "$scratch_dir/dbs" \
  --arg postgres_root "$postgres_data_root" \
  --arg local_mount "$local_mount_identity" \
  --arg postgres_mount "$postgres_mount_identity" \
  --argjson same_mount "$storage_same_mount" \
  --argjson durable "$storage_durable" \
  --arg bind_mode "$postgres_bind_mode" \
  --arg reserve_path "$reserve_path" \
  --argjson reserve_bytes "$reserve_bytes" \
  --argjson ci_trigger "$ci_trigger_json" \
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
      endpoint_scope:$endpoint_scope,
      container_id:($container_id | if length == 0 then null else . end),
      container_name:($container_name | if $backend == "docker_bind" then . else null end),
      container_run_id:($container_run_id | if $backend == "docker_bind" then . else null end),
      docker_daemon_id:($docker_daemon_id | if length == 0 then null else . end),
      endpoint_host:($endpoint_host | if length == 0 then null else . end),
      endpoint_port:$endpoint_port,
      rootfs_read_only:$rootfs_read_only,
      log_driver:$log_driver
    },
    storage:{
      class:$storage_class,
      local_database_root:$local_root,
      postgres_data_root:$postgres_root,
      local_mount_identity:$local_mount,
      postgres_mount_identity:$postgres_mount,
      same_mount:$same_mount,
      durable:$durable,
      postgres_bind_mode:$bind_mode,
      emergency_reserve_path:$reserve_path,
      emergency_reserve_bytes:$reserve_bytes
    },
    ci_trigger:$ci_trigger
  }' >"${evidence_file}.tmp"
mv "${evidence_file}.tmp" "$evidence_file"
case "$adversarial_case" in
  stale_container)
    jq '.postgres.container_id = ("0" * 64)' "$evidence_file" >"${evidence_file}.tmp"
    mv "${evidence_file}.tmp" "$evidence_file"
    ;;
  endpoint_mismatch)
    jq '.postgres.endpoint_port += 1' "$evidence_file" >"${evidence_file}.tmp"
    mv "${evidence_file}.tmp" "$evidence_file"
    ;;
esac
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
  exec timeout --signal=TERM --kill-after=30 "$cert_timeout" "$cert_bin" "${cert_args[@]}" \
    2>"$scratch_dir/cert.stderr.log"
) &
cert_pid=$!
wait "$cert_pid"
cert_status=$?
cert_pid=''
set -e

if [ -s "$scratch_dir/cert.stderr.log" ]; then
  cat "$scratch_dir/cert.stderr.log" >&2
fi

case "$cert_status" in
  124|137)
    annotate_progress deadline_exceeded timed_out deadline_exceeded
    ;;
esac

preserve_receipts
if [ -n "$adversarial_case" ]; then
  [ "$cert_status" -ne 0 ] || {
    printf 'adversarial case %s unexpectedly succeeded\n' "$adversarial_case" >&2
    exit 1
  }
  case "$adversarial_case" in
    stale_container)
      grep -F 'No such object' "$receipt_dir/cert.stderr.log" >/dev/null || \
        grep -F 'controlled docker inspect' "$receipt_dir/cert.stderr.log" >/dev/null || \
        grep -F 'storage-accounting container differs from execution evidence' \
          "$receipt_dir/cert.stderr.log" >/dev/null
      ;;
    endpoint_mismatch)
      grep -F 'is not the live container endpoint' "$receipt_dir/cert.stderr.log" >/dev/null
      ;;
    postgres_storage_overshoot)
      grep -F 'aggregate storage hard cap exceeded' "$receipt_dir/cert.stderr.log" >/dev/null
      ;;
  esac
  exit 0
fi
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
  .artifact_sha256 == .execution_evidence.binary_sha256 and
  (if .execution_evidence.postgres.backend == "docker_bind" then
     .live_postgres_observation.container_id == .execution_evidence.postgres.container_id and
     .live_postgres_observation.docker_daemon_id == .execution_evidence.postgres.docker_daemon_id and
     .live_postgres_observation.rootfs_read_only and
     .live_postgres_observation.log_driver == "none" and
     .live_postgres_observation.container_size_rw_bytes <= 1048576 and
     .live_postgres_observation.emergency_reserve_allocated_bytes >= .storage_contract.emergency_reserve_bytes
   else .live_postgres_observation == null end)
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
   .storage_contract == .execution_evidence.storage and
   (if .execution_evidence.postgres.backend == "docker_bind" then
      .live_postgres_observation.container_id == .execution_evidence.postgres.container_id and
      .live_postgres_observation.published_host_port == .execution_evidence.postgres.endpoint_port and
      .live_postgres_observation.rootfs_read_only and
      .live_postgres_observation.log_driver == "none" and
      .live_postgres_observation.container_size_rw_bytes <= 1048576
    else .live_postgres_observation == null end)' \
  "$receipt_dir/manifest.json" >/dev/null
jq -e --arg attempt_sha "$(sha256sum "$receipt_dir/attempt.json" | awk '{print $1}')" \
  '.attempt_receipt == "attempt.json" and .attempt_receipt_sha256 == $attempt_sha' \
  "$receipt_dir/manifest.json" >/dev/null
jq -e '. as $root | (($root.runs | length) > 0) and all($root.runs[]; .plan_integrity_verified and .reopen_verified)' \
  "$receipt_dir/raw-runs.json" >/dev/null
jq -e 'all(.runs[];
  if .engine == "postgres" then
    .engine_stats.storage_accounting_scope == "recursive_pgdata_apparent_bytes_plus_docker_size_rw"
  else true end
)' "$receipt_dir/raw-runs.json" >/dev/null
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
  jq -e '.status == "smoke_complete" and .mechanics_passed and
    (.release_eligible | not) and (.bounded_reference_win_eligible | not) and
    (.claim_scope | contains("no competitive, bounded-win, release, or customer-load claim"))' \
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
    .ci_trigger_bound and
    .storage_comparison_eligible and
    .storage_contract.class == "shared_host_durable_bind" and
    .bounded_reference_win_eligible
  ' \
    "$receipt_dir/manifest.json" >/dev/null
fi

exit "$cert_status"
