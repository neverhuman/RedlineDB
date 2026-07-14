#!/usr/bin/env bash
# Administrator preflight for the installed host-CI privilege boundary.
# Run only from the root-owned installed copy, never via sudo from a checkout.
set -euo pipefail
export PATH=/usr/bin:/bin
export LC_ALL=C

fail() {
  printf '[host-ci-preflight] %s\n' "$*" >&2
  exit 1
}

[[ "$(id -u)" == 0 ]] || fail 'must run as root'
install_dir="${1:-/usr/local/libexec/jain}"
install_dir="$(realpath -e -- "$install_dir")" || fail 'install directory missing'
[[ "$(stat -c '%u' -- "$install_dir")" == 0 \
  && "$((8#$(stat -c '%a' -- "$install_dir") & 8#022))" == 0 ]] \
  || fail 'install directory is not root-owned and write-protected'

publisher="$install_dir/host-ci-publisher"
publisher_config="$install_dir/host-ci-publisher.config.json"
sandbox="$install_dir/host-ci-sandbox"
sandbox_config="$install_dir/host-ci-sandbox.config.json"
for executable in "$publisher" "$sandbox"; do
  [[ ! -L "$executable" \
    && "$(stat -c '%u:%a:%h' -- "$executable" 2>/dev/null)" == '0:500:1' ]] \
    || fail "unsafe installed executable: $executable"
done
for config in "$publisher_config" "$sandbox_config"; do
  [[ ! -L "$config" \
    && "$(stat -c '%u:%a:%h' -- "$config" 2>/dev/null)" == '0:600:1' ]] \
    || fail "unsafe installed config: $config"
done
jq -e 'select(.schema_version == "jain.host-ci-publisher-config/v2")' \
  "$publisher_config" >/dev/null || fail 'invalid publisher config version'
jq -e 'select(.schema_version == "jain.host-ci-sandbox-config/v2")
  | select(.retain_requests == false)' "$sandbox_config" >/dev/null \
  || fail 'invalid or test-only sandbox config'
[[ "$(sha256sum -- "$publisher" | cut -d' ' -f1)" \
  == "$(jq -er '.publisher_sha256' "$publisher_config")" ]] \
  || fail 'publisher digest/config mismatch'
[[ "$(sha256sum -- "$sandbox" | cut -d' ' -f1)" \
  == "$(jq -er '.sandbox_sha256' "$sandbox_config")" ]] \
  || fail 'sandbox digest/config mismatch'
for field in \
  publisher_sha256 sandbox_sha256 control_remote forge_git_base request_root; do
  [[ "$(jq -er ".$field" "$publisher_config")" \
    == "$(jq -er ".$field" "$sandbox_config")" ]] \
    || fail "broker configs disagree on $field"
done

parent_uid="$(jq -er '.parent_uid' "$sandbox_config")"
parent_gid="$(jq -er '.parent_gid' "$sandbox_config")"
worker_user="$(jq -er '.worker_user' "$sandbox_config")"
worker_group="$(jq -er '.worker_group' "$sandbox_config")"
parent_user="$(getent passwd "$parent_uid" | cut -d: -f1)"
[[ -n "$parent_user" && "$(id -g "$parent_user")" == "$parent_gid" ]] \
  || fail 'configured parent identity is unavailable'
parent_home="$(getent passwd "$parent_user" | cut -d: -f6)"
[[ ! -e "$parent_home/.jeryu/secrets/merge-token" \
  && ! -L "$parent_home/.jeryu/secrets/merge-token" ]] \
  || fail 'legacy parent-readable forge credential must be revoked'
worker_record="$(getent passwd "$worker_user")" || fail 'worker user missing'
worker_uid="$(cut -d: -f3 <<<"$worker_record")"
worker_gid="$(getent group "$worker_group" | cut -d: -f3)"
worker_shell="$(cut -d: -f7 <<<"$worker_record")"
[[ "$worker_uid" != 0 && "$worker_gid" != 0 \
  && "$worker_shell" =~ /(nologin|false)$ ]] \
  || fail 'worker must be a non-root nologin service identity'
worker_sudo="$(sudo -n -l -U "$worker_user" 2>&1 || true)"
grep -Fq 'is not allowed to run sudo' <<<"$worker_sudo" \
  || fail 'worker has sudo authority'
parent_sudo="$(sudo -n -l -U "$parent_user" 2>&1)" \
  || fail 'cannot inspect parent sudo policy'
mapfile -t parent_rules < <(
  sed -n -E 's/^[[:space:]]+(\(.*)$/\1/p' <<<"$parent_sudo"
)
[[ "${#parent_rules[@]}" == 1 ]] \
  || fail 'parent sudo authority is not limited to the sandbox broker'
sandbox_rule=0
for rule in "${parent_rules[@]}"; do
  case "$rule" in
    "(root) NOPASSWD: $sandbox *") sandbox_rule=$((sandbox_rule + 1)) ;;
    *) fail "parent has unexpected sudo authority: $rule" ;;
  esac
done
[[ "$sandbox_rule" == 1 ]] || fail 'parent lacks the unique sandbox sudo rule'

worker_cache="$(realpath -e -- "$(jq -er '.worker_cache' "$sandbox_config")")" \
  || fail 'worker cache missing'
[[ "$(stat -c '%u:%g:%a' -- "$worker_cache")" \
  == "$worker_uid:$worker_gid:700" ]] \
  || fail 'worker cache ownership/mode mismatch'
request_root="$(realpath -e -- "$(jq -er '.request_root' "$sandbox_config")")" \
  || fail 'root request directory missing'
[[ "$(stat -c '%u:%g:%a' -- "$request_root")" == '0:0:700' ]] \
  || fail 'root request directory ownership/mode mismatch'
[[ "$(systemctl show -p Version --value)" =~ ^[0-9]+([.][0-9]+)*$ ]] \
  || fail 'systemd manager unavailable'
command -v systemd-run >/dev/null || fail 'systemd-run unavailable'
for launcher in /usr/bin/unshare /usr/bin/setpriv; do
  [[ ! -L "$launcher" \
    && "$(stat -c '%u:%a:%h' -- "$launcher" 2>/dev/null)" == '0:755:1' ]] \
    || fail "unsafe namespace launcher: $launcher"
done
host_pid_namespace="$(readlink /proc/self/ns/pid)" \
  || fail 'cannot identify the host PID namespace'
host_user_namespace="$(readlink /proc/self/ns/user)" \
  || fail 'cannot identify the host user namespace'
systemd-run --quiet --wait --collect --service-type=exec \
  --unit="jain-host-ci-preflight-$$" \
  --property="User=$worker_user" --property="Group=$worker_group" \
  --property=PrivateUsers=yes --property=PrivateNetwork=yes \
  --property=PrivateTmp=yes --property=ProtectProc=invisible \
  --property=ProtectSystem=strict --property=NoNewPrivileges=yes \
  --property='CapabilityBoundingSet=CAP_SYS_ADMIN CAP_SETPCAP' \
  --property='AmbientCapabilities=CAP_SYS_ADMIN CAP_SETPCAP' \
  --property='RestrictNamespaces=pid mnt' \
  --property='SystemCallFilter=@system-service unshare mount umount2' \
  --property='SystemCallFilter=~@resources @reboot @swap @module @raw-io @obsolete @keyring' \
  --setenv="JAIN_PREFLIGHT_HOST_PID_NAMESPACE=$host_pid_namespace" \
  --setenv="JAIN_PREFLIGHT_HOST_USER_NAMESPACE=$host_user_namespace" \
  /usr/bin/unshare --pid --fork --kill-child=KILL --mount-proc \
  /usr/bin/setpriv --inh-caps=-all --ambient-caps=-all \
    --bounding-set=-all --no-new-privs \
    /bin/bash -ceu '
      [[ "$(readlink /proc/self/ns/pid)" \
        != "${JAIN_PREFLIGHT_HOST_PID_NAMESPACE:?}" ]]
      [[ "$(readlink /proc/self/ns/user)" \
        != "${JAIN_PREFLIGHT_HOST_USER_NAMESPACE:?}" ]]
      awk '\''/^NoNewPrivs:/ { if ($2 != 1) exit 1 }'\'' /proc/self/status
      awk '\''/^Cap(Inh|Prm|Eff|Bnd|Amb):/ {
        if ($2 != "0000000000000000") exit 1
      }'\'' \
        /proc/self/status
    ' || fail 'kernel/systemd isolation probe failed'

printf 'host CI installed privilege boundary preflight ok\n'
