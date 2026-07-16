#!/usr/bin/env bash
# Unprivileged bootstrap for split-host-ci. Publication is not reachable here;
# the sole privileged transition is the exact root sandbox command.
set -uo pipefail
unset JAIN_BASE JAIN_HOST_CI_PUBLISHER
if [[ -v JERYU_BASE || -v JERYU_MERGE_TOKEN || -v JERYU_MERGE_TOKEN_FILE ]]; then
  unset JERYU_BASE JERYU_MERGE_TOKEN JERYU_MERGE_TOKEN_FILE
  printf '[split-host-ci] caller-provided forge credentials are forbidden\n' >&2
  exit 2
fi

OWNER="${1:?owner}"
REPO="${2:?repo}"
SHA="${3:?sha}"
REPO_PATH="${4:?repo_path}"
CHECK="${5:-$REPO/required}"
OPS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
split_root="$(realpath -e -- "${JAIN_SPLIT_ROOT:-/home/ubuntu/jain-split}")" || exit 2
control_commit="$("$OPS_ROOT/ops/ci/host-ci-integrity.sh" "$OPS_ROOT")" \
  || { printf '[split-host-ci] exact control-plane integrity check failed\n' >&2; exit 2; }
[[ "$SHA" =~ ^[0-9a-f]{40}$ ]] || exit 2

bootstrap_parent="$split_root/target/host-ci-sandboxes"
mkdir -p -- "$bootstrap_parent" || exit 2
bootstrap_parent="$(realpath -e -- "$bootstrap_parent")" || exit 2
[[ "$bootstrap_parent" == "$split_root/target/host-ci-sandboxes" \
  && ! -L "$bootstrap_parent" \
  && "$(stat -c '%u' -- "$bootstrap_parent")" == "$(id -u)" ]] || exit 2
# The private worker UID must traverse to the root-owned, separately bound
# bootstrap directory without being able to enumerate sibling attempts.
chmod 0711 -- "$bootstrap_parent" || exit 2
[[ "$(stat -c '%u:%a' -- "$bootstrap_parent")" == "$(id -u):711" ]] || exit 2
bootstrap_root="$(mktemp -d "$bootstrap_parent/split-host-ci-bootstrap.XXXXXX")" || exit 2
sandbox_request="$bootstrap_root/sandbox-request.json"
staged_product="$bootstrap_root/product-source"
child_log="$bootstrap_root/child.log"
cleanup() {
  local root_real
  root_real="$(realpath -e -- "$bootstrap_root" 2>/dev/null || true)"
  case "$root_real" in
    "$bootstrap_parent"/split-host-ci-bootstrap.??????)
      [[ "$root_real" == "$bootstrap_root" && ! -L "$root_real" \
        && "$(stat -c '%u:%a' -- "$root_real" 2>/dev/null)" \
          == "$(id -u):700" ]] && rm -rf -- "$root_real"
      ;;
  esac
}
trap cleanup EXIT

[[ -d "$REPO_PATH/.git" ]] || exit 2
mkdir -m 0700 "$bootstrap_root/child-home" "$bootstrap_root/writable" \
  "$bootstrap_root/cargo-target" || exit 2

child_environment='{}'
safe_child_vars=(
  CARGO_BUILD_JOBS CARGO_NET_OFFLINE RUSTFLAGS TERM
  JAIN_CI_JOBS JAIN_NEEDS_SIBLINGS JAIN_NEEDS_ARTIFACTS
  JAIN_NATIVE_SOURCE_ROOT
  CUDA_VISIBLE_DEVICES NVIDIA_VISIBLE_DEVICES NVIDIA_DRIVER_CAPABILITIES
  JAIN_TEST_ATTACK_URL JAIN_TEST_REQUIRE_ISOLATION JAIN_TEST_SLEEP_SECONDS
  JAIN_TEST_FORCE_FAILURE
  JAIN_TEST_ROOT_CONFIG_PATH JAIN_TEST_HOST_PID_NAMESPACE
  JAIN_TEST_HOST_USER_NAMESPACE JAIN_TEST_FS_MONITOR_PATH
  JAIN_TEST_ROOT_REQUEST_PATH
)
for child_var in "${safe_child_vars[@]}"; do
  if [[ -v "$child_var" ]]; then
    child_environment="$(jq -c --arg name "$child_var" \
      --arg value "${!child_var}" '. + {($name):$value}' \
      <<<"$child_environment")" || exit 2
  fi
done
child_environment="$(jq -c \
  --arg split_root "$split_root" \
  --arg target "$bootstrap_root/cargo-target" \
  --arg writable "$bootstrap_root/writable" \
  '. + {JAIN_SPLIT_ROOT:$split_root,CARGO_TARGET_DIR:$target,
    JAIN_HOST_CI_WRITABLE_ROOT:$writable,JAIN_RELEASE_CI:"1"}' \
  <<<"$child_environment")" || exit 2
jq -n --arg commit "$control_commit" \
  --arg split_root "$split_root" \
  --arg owner "$OWNER" --arg repo "$REPO" --arg sha "$SHA" \
  --arg product "$staged_product" --arg check "$CHECK" \
  --argjson environment "$child_environment" \
  '{schema_version:"jain.host-ci-sandbox-request/v4",
    control_plane_commit:$commit,split_root:$split_root,
    arguments:[$owner,$repo,$sha,$product,$check],environment:$environment}' \
  >"$sandbox_request" || exit 2
chmod 0600 "$sandbox_request"

sandbox_path="${JAIN_HOST_CI_SANDBOX:-/usr/local/libexec/jain/host-ci-sandbox}"
[[ "$sandbox_path" = /* ]] || exit 2
sandbox_path="$(realpath -e -- "$sandbox_path" 2>/dev/null)" \
  || { printf '[split-host-ci] root sandbox is unavailable\n' >&2; exit 2; }
[[ ! -L "$sandbox_path" \
  && "$(stat -c '%u:%a:%h' -- "$sandbox_path")" == '0:500:1' ]] \
  || { printf '[split-host-ci] root sandbox must be root-owned mode 0500\n' >&2; exit 2; }

runner_rc=0
/usr/bin/sudo -n "$sandbox_path" "$sandbox_request" >"$child_log" 2>&1 \
  || runner_rc=$?
chmod 0600 "$child_log" || exit 1
cat "$child_log" >&2
exit "$runner_rc"
