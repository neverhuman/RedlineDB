#!/usr/bin/env bash
# Root-owned systemd boundary and root-result sealer for split-host-ci.
set -euo pipefail

if [[ "${JAIN_HOST_CI_CLEAN_ENV:-0}" != 1 ]]; then
  exec /usr/bin/env -i PATH=/usr/bin:/bin LC_ALL=C \
    SUDO_UID="${SUDO_UID:-}" SUDO_GID="${SUDO_GID:-}" \
    JAIN_HOST_CI_CLEAN_ENV=1 /bin/bash "${BASH_SOURCE[0]}" "$@"
fi
export PATH=/usr/bin:/bin LC_ALL=C
unset CDPATH ENV BASH_ENV GIT_DIR GIT_WORK_TREE GIT_CONFIG_COUNT GIT_CONFIG
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
cd /

fail() {
  printf '[host-ci-sandbox] %s\n' "$*" >&2
  exit 1
}

[[ "$(id -u)" == 0 ]] || fail 'must run as root'
[[ "$#" == 1 ]] || fail 'expected one sandbox request path'
request="$1"

sandbox_path="$(realpath -e -- "${BASH_SOURCE[0]}")" \
  || fail 'cannot resolve sandbox path'
install_dir="$(dirname "$sandbox_path")"
config="$install_dir/host-ci-sandbox.config.json"
publisher_path="$install_dir/host-ci-publisher"
[[ ! -L "$sandbox_path" \
  && "$(stat -c '%u:%a:%h' -- "$sandbox_path" 2>/dev/null)" == '0:500:1' ]] \
  || fail 'sandbox must be root-owned mode 0500'
[[ ! -L "$publisher_path" \
  && "$(stat -c '%u:%a:%h' -- "$publisher_path" 2>/dev/null)" == '0:500:1' ]] \
  || fail 'publisher must be root-owned mode 0500'
[[ ! -L "$install_dir" && -d "$install_dir" \
  && "$(stat -c '%u' -- "$install_dir")" == 0 \
  && "$((8#$(stat -c '%a' -- "$install_dir") & 8#022))" == 0 ]] \
  || fail 'broker directory must be root-owned and write-protected'
[[ ! -L "$config" \
  && "$(stat -c '%u:%a:%h' -- "$config" 2>/dev/null)" == '0:600:1' ]] \
  || fail 'unsafe root sandbox config'
jq -e '
  select(.schema_version == "jain.host-ci-sandbox-config/v2")
  | select(.sandbox_sha256 | test("^[0-9a-f]{64}$"))
  | select(.publisher_sha256 | test("^[0-9a-f]{64}$"))
  | select(.parent_uid | type == "number")
  | select(.parent_gid | type == "number")
  | select(.worker_user | type == "string" and length > 0)
  | select(.worker_group | type == "string" and length > 0)
  | select(.family_root | type == "string" and startswith("/"))
  | select(.worker_cache | type == "string" and startswith("/"))
  | select(.cargo_bin | type == "string" and startswith("/"))
  | select(.rustup_home | type == "string" and startswith("/"))
  | select(.control_remote | type == "string" and length > 0)
  | select(.forge_git_base | type == "string" and length > 0)
  | select(.request_root | type == "string" and startswith("/"))
  | select(.retain_requests | type == "boolean")
  | select(.device_allow | type == "array")' "$config" >/dev/null \
  || fail 'invalid sandbox config schema'

sandbox_sha="$(sha256sum -- "$sandbox_path" | cut -d' ' -f1)"
publisher_sha="$(sha256sum -- "$publisher_path" | cut -d' ' -f1)"
[[ "$sandbox_sha" == "$(jq -er '.sandbox_sha256' "$config")" \
  && "$publisher_sha" == "$(jq -er '.publisher_sha256' "$config")" ]] \
  || fail 'installed broker digest/config mismatch'

parent_uid="$(jq -er '.parent_uid' "$config")"
parent_gid="$(jq -er '.parent_gid' "$config")"
[[ "${SUDO_UID:-}" == "$parent_uid" && "${SUDO_GID:-}" == "$parent_gid" ]] \
  || fail 'sandbox caller does not match configured parent identity'
worker_user="$(jq -er '.worker_user' "$config")"
worker_group="$(jq -er '.worker_group' "$config")"
worker_record="$(getent passwd "$worker_user")" \
  || fail 'dedicated worker user is unavailable'
worker_uid="$(cut -d: -f3 <<<"$worker_record")"
worker_gid="$(getent group "$worker_group" | cut -d: -f3)"
worker_shell="$(cut -d: -f7 <<<"$worker_record")"
[[ "$worker_uid" =~ ^[0-9]+$ && "$worker_uid" != 0 \
  && "$worker_gid" =~ ^[0-9]+$ && "$worker_gid" != 0 \
  && "$worker_shell" =~ /(nologin|false)$ ]] \
  || fail 'worker must be a non-root nologin identity'
worker_sudo="$(sudo -n -l -U "$worker_user" 2>&1 || true)"
grep -Fq 'is not allowed to run sudo' <<<"$worker_sudo" \
  || fail 'worker unexpectedly has sudo authority'

family_root="$(realpath -e -- "$(jq -er '.family_root' "$config")")" \
  || fail 'family root unavailable'
worker_cache="$(realpath -e -- "$(jq -er '.worker_cache' "$config")")" \
  || fail 'worker cache unavailable'
cargo_bin="$(realpath -e -- "$(jq -er '.cargo_bin' "$config")")" \
  || fail 'Cargo bin directory unavailable'
rustup_home="$(realpath -e -- "$(jq -er '.rustup_home' "$config")")" \
  || fail 'rustup home unavailable'
request_root="$(realpath -e -- "$(jq -er '.request_root' "$config")")" \
  || fail 'root request directory unavailable'
[[ "$(stat -c '%u:%a' -- "$worker_cache")" == "$worker_uid:700" \
  && "$(stat -c '%u:%a' -- "$request_root")" == '0:700' ]] \
  || fail 'cache or root request ownership mismatch'
for launcher in /usr/bin/unshare /usr/bin/setpriv; do
  [[ ! -L "$launcher" \
    && "$(stat -c '%u:%a:%h' -- "$launcher")" == '0:755:1' ]] \
    || fail "unsafe namespace launcher: $launcher"
done

request="$(realpath -e -- "$request")" || fail 'sandbox request missing'
bootstrap_root="$(dirname "$request")"
case "$bootstrap_root" in
  /tmp/split-host-ci-bootstrap.??????) ;;
  *) fail 'request is outside a host-CI bootstrap directory' ;;
esac
[[ "$request" == "$bootstrap_root/sandbox-request.json" \
  && ! -L "$request" && ! -L "$bootstrap_root" \
  && "$(stat -c '%u:%g:%a:%h' -- "$request")" \
    == "$parent_uid:$parent_gid:600:1" \
  && "$(stat -c '%u:%g:%a' -- "$bootstrap_root")" \
    == "$parent_uid:$parent_gid:700" ]] \
  || fail 'unsafe sandbox request ownership or location'
jq -e '
  select(.schema_version == "jain.host-ci-sandbox-request/v2")
  | select(.control_plane_commit | test("^[0-9a-f]{40}$"))
  | select(.split_root | type == "string" and startswith("/"))
  | select(.splitctl_path | type == "string" and startswith("/"))
  | select(.splitctl_sha256 | test("^[0-9a-f]{64}$"))
  | select(.arguments | type == "array" and length == 5)
  | select(.environment | type == "object")
  | select(.environment | all(to_entries[]; .value | type == "string"))' \
  "$request" >/dev/null || fail 'invalid sandbox request schema'
control_commit="$(jq -er '.control_plane_commit' "$request")"
split_root="$(realpath -e -- "$(jq -er '.split_root' "$request")")" \
  || fail 'split root unavailable'
[[ "$split_root" == "$family_root" ]] || fail 'split root is not configured authority'
splitctl_input="$(realpath -e -- "$(jq -er '.splitctl_path' "$request")")" \
  || fail 'splitctl input unavailable'
[[ "$splitctl_input" == "$bootstrap_root/splitctl" \
  && ! -L "$splitctl_input" \
  && "$(stat -c '%u:%g:%a:%h' -- "$splitctl_input")" \
    == "$parent_uid:$parent_gid:500:1" \
  && "$(sha256sum -- "$splitctl_input" | cut -d' ' -f1)" \
    == "$(jq -er '.splitctl_sha256' "$request")" ]] \
  || fail 'unsafe splitctl input'
mapfile -t arguments < <(jq -er '.arguments[]' "$request")
[[ "${#arguments[@]}" == 5 \
  && "${arguments[0]}" =~ ^[a-z0-9][a-z0-9-]*$ \
  && "${arguments[1]}" =~ ^[a-z0-9][a-z0-9-]*$ \
  && "${arguments[2]}" =~ ^[0-9a-f]{40}$ \
  && "${arguments[3]}" == "$bootstrap_root/product-source" \
  && "${arguments[4]}" =~ ^[a-z0-9][a-z0-9-]*/required$ \
  && ! -e "${arguments[3]}" && ! -L "${arguments[3]}" ]] \
  || fail 'invalid runner arguments'

allowed_environment='^(CARGO_BUILD_JOBS|CARGO_NET_OFFLINE|CARGO_TARGET_DIR|RUSTFLAGS|TERM|JAIN_[A-Z0-9_]+|CUDA_VISIBLE_DEVICES|NVIDIA_VISIBLE_DEVICES|NVIDIA_DRIVER_CAPABILITIES)$'
mapfile -t environment_names < <(jq -r '.environment | keys[]' "$request")
for name in "${environment_names[@]}"; do
  [[ "$name" =~ $allowed_environment \
    && "$name" != *TOKEN* && "$name" != *SECRET* && "$name" != *PASSWORD* \
    && "$name" != JAIN_BASE && "$name" != JAIN_HOST_CI_PUBLISHER \
    && "$name" != JAIN_HOST_CI_HOST_PID_NAMESPACE \
    && "$name" != JAIN_HOST_CI_HOST_USER_NAMESPACE ]] \
    || fail "forbidden sandbox environment key: $name"
done
[[ "$(jq -er '.environment.CARGO_TARGET_DIR' "$request")" \
  == "$bootstrap_root/cargo-target" \
  && "$(jq -er '.environment.JAIN_HOST_CI_WRITABLE_ROOT' "$request")" \
    == "$bootstrap_root/writable" \
  && "$(jq -er '.environment.JAIN_SPLIT_ROOT' "$request")" == "$family_root" ]] \
  || fail 'worker writable paths are outside bootstrap authority'

request_id="$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')"
nonce="$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')"
[[ "$request_id" =~ ^[0-9a-f]{64}$ && "$nonce" =~ ^[0-9a-f]{64}$ ]] \
  || fail 'root randomness unavailable'
root_request="$request_root/$request_id"
mkdir -m 0700 "$root_request" || fail 'cannot create root request'
worker_authority="$root_request/worker-authority"
control_root="$worker_authority/control-plane"
mkdir -m 0755 "$worker_authority"

retain_requests="$(jq -er '.retain_requests' "$config")"
cleanup_root_request() {
  if [[ "$retain_requests" != true ]]; then
    rm -rf -- "$root_request"
  fi
}
trap cleanup_root_request EXIT

safe_git=(git -c core.fsmonitor=false -c core.hooksPath=/dev/null \
  -c core.untrackedCache=false -c diff.external=)
control_remote="$(jq -er '.control_remote' "$config")"
reviewed_commit="$("${safe_git[@]}" ls-remote --exit-code \
  "$control_remote" refs/heads/main 2>/dev/null | cut -f1)" \
  || fail 'cannot read configured control-plane main'
[[ "$reviewed_commit" == "$control_commit" ]] \
  || fail 'requested control commit is not reviewed main'
"${safe_git[@]}" init --quiet "$control_root"
"${safe_git[@]}" -C "$control_root" remote add origin "$control_remote"
"${safe_git[@]}" -C "$control_root" fetch --quiet --no-tags \
  "$control_remote" refs/heads/main
"${safe_git[@]}" -C "$control_root" checkout --quiet --detach FETCH_HEAD
[[ "$("${safe_git[@]}" -C "$control_root" rev-parse 'HEAD^{commit}')" \
  == "$control_commit" ]] || fail 'root immutable checkout mismatch'
[[ "$(sha256sum -- "$control_root/ops/ci/host-ci-sandbox.sh" | cut -d' ' -f1)" \
  == "$sandbox_sha" \
  && "$(sha256sum -- "$control_root/ops/ci/host-ci-publisher.sh" | cut -d' ' -f1)" \
    == "$publisher_sha" ]] \
  || fail 'installed brokers do not match reviewed main'

# Resolve owner/check only from the root-fetched manifest.
repo="${arguments[1]}"
repo_authority="$({
  awk -v wanted="$repo" '
    function val(line, value) {
      value=line; sub(/^[^=]*=[[:space:]]*"/,"",value)
      sub(/"[[:space:]]*$/,"",value); return value
    }
    function finish() {
      if (!active || name != wanted) return
      count++; final_check=check; final_owner=(forge_owner==""?"jeryu":forge_owner)
    }
    $0 == "[control_plane]" {
      finish(); active=(wanted=="jain-split-ops"); name="jain-split-ops"
      check=""; forge_owner="jeryu"; next
    }
    $0 == "[[repo]]" || $0 == "[[infrastructure_repo]]" {
      finish(); active=1; name=""; check=""; forge_owner=""; next
    }
    /^\[\[/ { finish(); active=0; next }
    /^\[/ { finish(); active=0; next }
    active && /^[[:space:]]*name[[:space:]]*=/ { name=val($0); next }
    active && /^[[:space:]]*required_check[[:space:]]*=/ { check=val($0); next }
    active && /^[[:space:]]*forge_owner[[:space:]]*=/ { forge_owner=val($0); next }
    END {
      finish(); if (count != 1 || final_check == "") exit 1
      printf "%s\t%s\n", final_owner, final_check
    }
  ' "$control_root/repos.manifest.toml"
} 2>/dev/null)" || fail 'repository authority is absent or ambiguous'
IFS=$'\t' read -r protected_owner protected_check <<<"$repo_authority"
[[ "${arguments[0]}" == "$protected_owner" \
  && "${arguments[4]}" == "$protected_check" ]] \
  || fail 'requested owner/check differs from manifest authority'

# Never run a caller-supplied product checkout. Resolve an advertised ref from
# the configured forge Git root into root-owned storage, then stage an
# independent detached clone for the worker before changing bootstrap ownership.
forge_git_base="$(jq -er '.forge_git_base' "$config")"
product_remote="${forge_git_base%/}/$protected_owner/$repo.git"
product_refs="$("${safe_git[@]}" ls-remote --exit-code \
  "$product_remote" 2>/dev/null)" \
  || fail 'cannot read authoritative product refs'
product_ref="$(awk -v head="${arguments[2]}" '
    $1 == head && !found { ref=$2; sub(/\^\{\}$/,"",ref); found=1 }
    END { if (!found) exit 1; print ref }
  ' <<<"$product_refs")" \
  || fail 'requested head is not an advertised product ref'
[[ "$product_ref" == HEAD || "$product_ref" == refs/* ]] \
  || fail 'requested head is not an advertised product ref'
product_authority="$root_request/product-authority"
"${safe_git[@]}" init --quiet "$product_authority"
"${safe_git[@]}" -C "$product_authority" fetch --quiet --no-tags \
  "$product_remote" "$product_ref"
"${safe_git[@]}" -C "$product_authority" checkout --quiet --detach FETCH_HEAD
[[ "$("${safe_git[@]}" -C "$product_authority" rev-parse 'HEAD^{commit}')" \
  == "${arguments[2]}" ]] || fail 'root product checkout commit mismatch'
git clone --quiet --no-local --no-checkout \
  "$product_authority" "${arguments[3]}"
git -C "${arguments[3]}" checkout --quiet --detach "${arguments[2]}"

install -o root -g root -m 0555 "$splitctl_input" "$worker_authority/splitctl"
install -o root -g root -m 0555 \
  "$control_root/ops/ci/split-host-ci.sh" \
  "$worker_authority/.split-host-ci-reviewed"
worker_result="$bootstrap_root/writable/worker-evidence.json"
jq -n --arg commit "$control_commit" --arg result "$worker_result" \
  '{schema_version:"jain.host-ci-reexec/v2",
    source_root:"/opt/jain-ci/authority/control-plane",
    exact_root:"/opt/jain-ci/authority/control-plane",
    commit:$commit,result_path:$result,
    splitctl_path:"/opt/jain-ci/authority/splitctl"}' \
  >"$worker_authority/reexec-state.json"
chmod 0444 "$worker_authority/reexec-state.json"
chown root:root "$worker_authority/reexec-state.json"
chmod -R go-w "$control_root"
chown -R root:root "$worker_authority"

created_at="$(date +%s)"
root_state="$root_request/root-state.json"
jq -n --arg request_id "$request_id" --arg nonce "$nonce" \
  --arg commit "$control_commit" --arg remote "$control_remote" \
  --arg publisher_sha "$publisher_sha" --arg sandbox_sha "$sandbox_sha" \
  --argjson created_at "$created_at" \
  '{schema_version:"jain.host-ci-root-state/v2",status:"running",
    request_id:$request_id,nonce:$nonce,created_at:$created_at,
    control_plane_commit:$commit,control_remote:$remote,
    publisher_sha256:$publisher_sha,sandbox_sha256:$sandbox_sha}' >"$root_state"
chmod 0600 "$root_state"
chown root:root "$root_state"

unit="jain-host-ci-${request_id:0:24}.service"
restore_owner() {
  systemctl kill --kill-whom=all --signal=KILL "$unit" >/dev/null 2>&1 || true
  systemctl reset-failed "$unit" >/dev/null 2>&1 || true
  chown -R "$parent_uid:$parent_gid" "$bootstrap_root" >/dev/null 2>&1 || true
}
trap 'restore_owner; cleanup_root_request' EXIT
chown -R "$worker_uid:$worker_gid" "$bootstrap_root"

host_pid_namespace="$(readlink /proc/self/ns/pid)"
host_user_namespace="$(readlink /proc/self/ns/user)"
systemd_args=(
  --quiet --wait --pipe --collect --service-type=exec --unit="$unit"
  --property="User=$worker_user" --property="Group=$worker_group"
  --property=PrivateUsers=yes --property=PrivateNetwork=yes
  --property=PrivateTmp=yes --property=ProtectProc=invisible
  --property=ProcSubset=pid --property=ProtectSystem=strict
  --property=ProtectHome=tmpfs --property=NoNewPrivileges=yes
  --property='CapabilityBoundingSet=CAP_SYS_ADMIN CAP_SETPCAP'
  --property='AmbientCapabilities=CAP_SYS_ADMIN CAP_SETPCAP'
  --property=RestrictSUIDSGID=yes --property=LockPersonality=yes
  --property=RestrictRealtime=yes --property='RestrictNamespaces=pid mnt'
  --property=SystemCallArchitectures=native
  --property='SystemCallFilter=@system-service unshare mount umount2'
  --property='SystemCallFilter=~@resources @reboot @swap @module @raw-io @obsolete @keyring'
  --property=SystemCallErrorNumber=EPERM
  --property=RestrictAddressFamilies=AF_UNIX
  --property=DevicePolicy=closed --property=PrivateDevices=no
  --property=KillMode=control-group --property=SendSIGKILL=yes
  --property=TimeoutStopSec=5s
  --property="BindPaths=$bootstrap_root"
  --property="BindPaths=$worker_cache:/opt/jain-ci/cargo-home"
  --property="BindReadOnlyPaths=$worker_authority:/opt/jain-ci/authority"
  --property="BindReadOnlyPaths=$family_root"
  --property="BindReadOnlyPaths=$cargo_bin:/opt/jain-ci/cargo-bin"
  --property="BindReadOnlyPaths=$rustup_home:/opt/jain-ci/rustup"
  --property="InaccessiblePaths=/usr/bin/sudo /etc/sudoers /etc/sudoers.d -$install_dir -$request_root"
  --setenv="HOME=$bootstrap_root/child-home"
  --setenv="USER=$worker_user" --setenv="LOGNAME=$worker_user"
  --setenv=SHELL=/bin/bash
  --setenv=PATH=/opt/jain-ci/cargo-bin:/usr/bin:/bin
  --setenv=CARGO_HOME=/opt/jain-ci/cargo-home
  --setenv=RUSTUP_HOME=/opt/jain-ci/rustup
  --setenv=JAIN_HOST_CI_REEXEC_STATE=/opt/jain-ci/authority/reexec-state.json
  --setenv=GIT_CONFIG_COUNT=3
  --setenv=GIT_CONFIG_KEY_0=safe.directory
  --setenv=GIT_CONFIG_VALUE_0=/opt/jain-ci/authority/control-plane
  --setenv=GIT_CONFIG_KEY_1=core.fsmonitor
  --setenv=GIT_CONFIG_VALUE_1=false
  --setenv=GIT_CONFIG_KEY_2=core.hooksPath
  --setenv=GIT_CONFIG_VALUE_2=/dev/null
  --setenv="JAIN_HOST_CI_HOST_PID_NAMESPACE=$host_pid_namespace"
  --setenv="JAIN_HOST_CI_HOST_USER_NAMESPACE=$host_user_namespace"
  --setenv=JAIN_HOST_CI_NETWORK_ISOLATED=1
)
while IFS=$'\t' read -r name value; do
  systemd_args+=(--setenv="$name=$value")
done < <(jq -r '.environment | to_entries[] | [.key, .value] | @tsv' "$request")
while IFS= read -r device; do
  [[ "$device" =~ ^/dev/nvidia[a-zA-Z0-9_-]*[[:space:]]+(r|rw|rwm)$ ]] \
    || fail "invalid GPU device allowance: $device"
  systemd_args+=(--property="DeviceAllow=$device")
done < <(jq -r '.device_allow[]' "$config")

runner_rc=0
systemd-run "${systemd_args[@]}" \
  /usr/bin/unshare --pid --fork --kill-child=KILL --mount-proc \
  /usr/bin/setpriv --inh-caps=-all --ambient-caps=-all \
    --bounding-set=-all --no-new-privs \
    /bin/bash -ceu '
      [[ "$(readlink /proc/self/ns/pid)" \
        != "${JAIN_HOST_CI_HOST_PID_NAMESPACE:?}" ]]
      [[ "$(readlink /proc/self/ns/user)" \
        != "${JAIN_HOST_CI_HOST_USER_NAMESPACE:?}" ]]
      unset JAIN_HOST_CI_HOST_PID_NAMESPACE JAIN_HOST_CI_HOST_USER_NAMESPACE
      printf "[host-ci-sandbox] nested PID/user namespaces established\n" >&2
      exec /bin/bash "$@"
    ' -- /opt/jain-ci/authority/.split-host-ci-reviewed \
      "${arguments[@]}" || runner_rc=$?
if [[ "$runner_rc" != 0 ]]; then
  journalctl --quiet --no-pager --unit "$unit" --lines=80 >&2 || true
fi
systemctl kill --kill-whom=all --signal=KILL "$unit" >/dev/null 2>&1 || true
systemctl is-active --quiet "$unit" \
  && fail 'sandbox cgroup remained active after worker exit'
printf '[host-ci-sandbox] worker cgroup stopped before sealing\n' >&2

conclusion=failure
evidence_dir=''
evidence_sha=''
if [[ "$runner_rc" == 0 && -f "$worker_result" && ! -L "$worker_result" \
  && "$(stat -c '%u:%a:%h' -- "$worker_result")" == "$worker_uid:600:1" ]] \
  && jq -e --arg owner "${arguments[0]}" --arg repo "$repo" \
    --arg head "${arguments[2]}" --arg check "${arguments[4]}" \
    --arg commit "$control_commit" \
    'select(.schema_version == "jain.host-ci-worker-evidence/v2")
     | select(.owner == $owner and .repository == $repo)
     | select(.head_sha == $head and .required_check == $check)
     | select(.control_plane_commit == $commit)
     | select(.native_evidence_dir | type == "string")
     | select(.native_evidence_sha256 | type == "string")' \
    "$worker_result" >/dev/null; then
  evidence_dir="$(jq -er '.native_evidence_dir' "$worker_result")"
  evidence_sha="$(jq -er '.native_evidence_sha256' "$worker_result")"
  conclusion=success
fi

# shellcheck source=ops/ci/native-runtime.sh
source "$control_root/ops/ci/native-runtime.sh"
derived_required=false
if jain_native_check_requires_evidence "$repo" "${arguments[4]}" "$protected_check"; then
  derived_required=true
fi
if [[ "$conclusion" == success ]]; then
  required_int=0
  [[ "$derived_required" == true ]] && required_int=1
  if ! jain_verify_native_check_evidence success "$required_int" \
    "$evidence_dir" "$evidence_sha" "${arguments[2]}" "${arguments[4]}" \
    "$control_root" "$control_commit"; then
    printf '[host-ci-sandbox] trusted native evidence policy rejected worker output\n' >&2
    conclusion=failure
  fi
fi

root_result="$root_request/root-result.json"
jq -n --arg request_id "$request_id" --arg commit "$control_commit" \
  --arg owner "${arguments[0]}" --arg repo "$repo" \
  --arg head "${arguments[2]}" --arg check "${arguments[4]}" \
  --arg conclusion "$conclusion" --arg evidence_dir "$evidence_dir" \
  --arg evidence_sha "$evidence_sha" --argjson rc "$runner_rc" \
  --argjson evidence_required "$derived_required" \
  '{schema_version:"jain.host-ci-root-result/v2",request_id:$request_id,
    control_plane_commit:$commit,owner:$owner,repository:$repo,head_sha:$head,
    required_check:$check,conclusion:$conclusion,runner_exit_code:$rc,
    native_evidence_required:$evidence_required,
    native_evidence_dir:$evidence_dir,native_evidence_sha256:$evidence_sha}' \
  >"$root_result"
chmod 0600 "$root_result"
chown root:root "$root_result"
result_sha="$(sha256sum -- "$root_result" | cut -d' ' -f1)"
sealed_at="$(date +%s)"
root_seal="$({
  printf '%s\n%s\n%s\n%s\n' "$nonce" "$result_sha" "$sealed_at" "$request_id"
} | sha256sum | cut -d' ' -f1)"
jq --arg status sealed --arg result_sha "$result_sha" \
  --arg root_seal "$root_seal" --argjson sealed_at "$sealed_at" \
  '. + {status:$status,result_sha256:$result_sha,
    root_seal:$root_seal,sealed_at:$sealed_at}' "$root_state" \
  >"$root_request/root-state.tmp"
chmod 0600 "$root_request/root-state.tmp"
chown root:root "$root_request/root-state.tmp"
mv -f -- "$root_request/root-state.tmp" "$root_state"

publish_rc=0
"$publisher_path" "$root_request" || publish_rc=$?
chown -R "$parent_uid:$parent_gid" "$bootstrap_root"
trap cleanup_root_request EXIT
if [[ "$publish_rc" != 0 || "$conclusion" != success ]]; then
  exit 1
fi
exit 0
