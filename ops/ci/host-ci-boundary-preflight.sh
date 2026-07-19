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

validate_control_authority() {
  local ref="$1" commit="$2" expires="$3" now
  if [[ "$ref" == refs/heads/main ]]; then
    [[ -z "$commit" && -z "$expires" ]] \
      || fail 'production control authority cannot carry bootstrap fields'
    return
  fi
  [[ "$ref" =~ ^refs/heads/[a-z0-9][a-z0-9._/-]*[a-z0-9]$ \
    && "$ref" != *..* && "$ref" != *//* && "$ref" != *@\{* \
    && "$ref" != *.lock && "$commit" =~ ^[0-9a-f]{40}$ \
    && "$expires" =~ ^[0-9]+$ ]] \
    || fail 'invalid bootstrap control authority'
  now="$(date +%s)"
  (( expires >= now && expires - now <= 6900 )) \
    || fail 'bootstrap control authority is expired or exceeds 6,900 seconds'
}

validate_cargo_registry_cache() {
  local root="${1:?Cargo registry cache is required}" path metadata
  case "$root" in
    /tmp | /tmp/*) fail 'Cargo registry cache cannot use /tmp' ;;
  esac
  [[ -d "$root" && ! -L "$root" \
    && "$(stat -c '%u:%g:%a' -- "$root" 2>/dev/null)" == '0:0:555' \
    && -d "$root/cache" && -d "$root/index" ]] \
    || fail 'Cargo registry cache root is not immutable root authority'
  find "$root" -xdev -print >/dev/null \
    || fail 'Cargo registry cache cannot be traversed'
  while IFS= read -r -d '' path; do
    metadata="$(stat -c '%u:%g:%a' -- "$path" 2>/dev/null)" \
      || fail "cannot inspect Cargo registry cache directory: $path"
    [[ "$metadata" == '0:0:555' ]] \
      || fail "unsafe Cargo registry cache directory: $path"
  done < <(find "$root" -xdev -type d -print0)
  while IFS= read -r -d '' path; do
    metadata="$(stat -c '%u:%g:%a:%h' -- "$path" 2>/dev/null)" \
      || fail "cannot inspect Cargo registry cache file: $path"
    [[ "$metadata" == '0:0:444:1' ]] \
      || fail "unsafe Cargo registry cache file: $path"
  done < <(find "$root" -xdev -type f -print0)
  [[ -z "$(find "$root" -xdev ! -type d ! -type f -print -quit)" \
    && -n "$(find "$root/cache" -mindepth 1 -maxdepth 1 -type d -print -quit)" \
    && -n "$(find "$root/index" -mindepth 1 -maxdepth 1 -type d -print -quit)" ]] \
    || fail 'Cargo registry cache has unsafe nodes or incomplete roots'
}

validate_grype_db() {
  local root="$1" expected="$2" relative path inventory_digest actual_nodes
  case "$root" in
    /tmp | /tmp/*) fail 'Grype database cannot use /tmp' ;;
  esac
  [[ -d "$root" && ! -L "$root" \
    && "$(stat -c '%u:%g:%a' -- "$root" 2>/dev/null)" == '0:0:555' \
    && "$(stat -c '%u:%g:%a' -- "$root/6" 2>/dev/null)" == '0:0:555' ]] \
    || fail 'Grype database directories are not immutable root authority'
  actual_nodes="$(find "$root" -mindepth 1 -printf '%P\n' | sort)"
  [[ "$actual_nodes" == $'6\n6/import.json\n6/vulnerability.db' \
    && -z "$(find "$root" -mindepth 1 ! -type d ! -type f -print -quit)" ]] \
    || fail 'Grype database inventory is not closed'
  inventory_digest="$({
    for relative in 6/import.json 6/vulnerability.db; do
      path="$root/$relative"
      [[ "$(stat -c '%u:%g:%a:%h' -- "$path" 2>/dev/null)" == '0:0:444:1' ]] \
        || fail "unsafe Grype database file: $relative"
      printf '%s\t%s\t%s\n' "$relative" "$(stat -c %s -- "$path")" \
        "$(sha256sum -- "$path" | cut -d' ' -f1)"
    done
  } | sha256sum | cut -d' ' -f1)"
  [[ "$inventory_digest" == "$expected" ]] \
    || fail 'Grype database inventory digest mismatch'
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
splitctl="$install_dir/splitctl"
jankurai="$install_dir/jankurai"
for executable in "$publisher" "$sandbox" "$splitctl"; do
  [[ ! -L "$executable" \
    && "$(stat -c '%u:%a:%h' -- "$executable" 2>/dev/null)" == '0:500:1' ]] \
    || fail "unsafe installed executable: $executable"
done
[[ ! -L "$jankurai" \
  && "$(stat -c '%u:%a:%h' -- "$jankurai" 2>/dev/null)" == '0:555:1' ]] \
  || fail 'unsafe installed Jankurai auditor'
for config in "$publisher_config" "$sandbox_config"; do
  [[ ! -L "$config" \
    && "$(stat -c '%u:%a:%h' -- "$config" 2>/dev/null)" == '0:600:1' ]] \
    || fail "unsafe installed config: $config"
done
jq -e 'select(.schema_version == "jain.host-ci-publisher-config/v5")
  | select(.jankurai_sha256 | test("^[0-9a-f]{64}$"))
  | select((.security_tool_sha256 | keys) == ["actionlint", "grype", "syft"])
  | select(all(.security_tool_sha256[]; test("^[0-9a-f]{64}$")))
  | select(.grype_db_root | type == "string" and startswith("/"))
  | select(.grype_db_inventory_sha256 | test("^[0-9a-f]{64}$"))
  | select(.proof_evidence_root | type == "string" and startswith("/"))
  | select(.token_file | type == "string" and startswith("/"))
  | select((.control_ref // "refs/heads/main") | type == "string")
  | select((.bootstrap_commit // "") | type == "string")
  | select((.bootstrap_expires_at // "") | type == "string")
  | select(has("token") | not)' \
  "$publisher_config" >/dev/null || fail 'invalid publisher config version'
token_file="$(jq -er '.token_file' "$publisher_config")"
[[ ! -L "$token_file" \
  && "$(realpath -e -- "$token_file" 2>/dev/null)" == "$token_file" \
  && "$(stat -c '%u:%g:%a:%h' -- "$token_file" 2>/dev/null)" == '0:0:600:1' ]] \
  || fail 'publisher token file must be canonical root:root mode 0600 single-link'
jq -e 'select(.schema_version == "jain.host-ci-sandbox-config/v5")
  | select(.jankurai_sha256 | test("^[0-9a-f]{64}$"))
  | select((.security_tool_sha256 | keys) == ["actionlint", "grype", "syft"])
  | select(all(.security_tool_sha256[]; test("^[0-9a-f]{64}$")))
  | select(.cargo_registry_cache | type == "string" and startswith("/"))
  | select(.grype_db_root | type == "string" and startswith("/"))
  | select(.grype_db_inventory_sha256 | test("^[0-9a-f]{64}$"))
  | select(.proof_evidence_root | type == "string" and startswith("/"))
  | select(.token_file | type == "string" and startswith("/"))
  | select((.control_ref // "refs/heads/main") | type == "string")
  | select((.bootstrap_commit // "") | type == "string")
  | select((.bootstrap_expires_at // "") | type == "string")
  | select(has("token") | not)
  | select(.retain_requests == false)' "$sandbox_config" >/dev/null \
  || fail 'invalid or test-only sandbox config'
control_ref="$(jq -er '.control_ref // "refs/heads/main"' "$sandbox_config")"
bootstrap_commit="$(jq -er '.bootstrap_commit // ""' "$sandbox_config")"
bootstrap_expires_at="$(jq -er '.bootstrap_expires_at // ""' "$sandbox_config")"
validate_control_authority \
  "$control_ref" "$bootstrap_commit" "$bootstrap_expires_at"
[[ "$(jq -er '.control_ref // "refs/heads/main"' "$publisher_config")" == "$control_ref" \
  && "$(jq -er '.bootstrap_commit // ""' "$publisher_config")" == "$bootstrap_commit" \
  && "$(jq -er '.bootstrap_expires_at // ""' "$publisher_config")" \
    == "$bootstrap_expires_at" ]] \
  || fail 'broker configs disagree on control bootstrap authority'
[[ "$(sha256sum -- "$publisher" | cut -d' ' -f1)" \
  == "$(jq -er '.publisher_sha256' "$publisher_config")" ]] \
  || fail 'publisher digest/config mismatch'
[[ "$(sha256sum -- "$sandbox" | cut -d' ' -f1)" \
  == "$(jq -er '.sandbox_sha256' "$sandbox_config")" ]] \
  || fail 'sandbox digest/config mismatch'
[[ "$(sha256sum -- "$splitctl" | cut -d' ' -f1)" \
  == "$(jq -er '.splitctl_sha256' "$sandbox_config")" ]] \
  || fail 'splitctl digest/config mismatch'
[[ "$(sha256sum -- "$jankurai" | cut -d' ' -f1)" \
  == "$(jq -er '.jankurai_sha256' "$sandbox_config")" \
  && "$("$jankurai" --version)" == 'jankurai 1.6.11' ]] \
  || fail 'governed Jankurai binary/version mismatch'
for tool in actionlint grype syft; do
  tool_path="$install_dir/security-$tool"
  [[ ! -L "$tool_path" \
    && "$(stat -c '%u:%g:%a:%h' -- "$tool_path" 2>/dev/null)" == '0:0:555:1' \
    && "$(sha256sum -- "$tool_path" | cut -d' ' -f1)" \
      == "$(jq -er --arg tool "$tool" '.security_tool_sha256[$tool]' "$sandbox_config")" ]] \
    || fail "installed security tool digest/metadata mismatch: $tool"
done
[[ "$(jq -c '.security_tool_sha256' "$publisher_config")" \
  == "$(jq -c '.security_tool_sha256' "$sandbox_config")" ]] \
  || fail 'broker configs disagree on security tool authority'
for field in \
  publisher_sha256 sandbox_sha256 splitctl_sha256 jankurai_sha256 \
  control_remote forge_git_base request_root native_evidence_root \
  proof_evidence_root token_file grype_db_root grype_db_inventory_sha256; do
  [[ "$(jq -er ".$field" "$publisher_config")" \
    == "$(jq -er ".$field" "$sandbox_config")" ]] \
    || fail "broker configs disagree on $field"
done

parent_uid="$(jq -er '.parent_uid' "$sandbox_config")"
parent_gid="$(jq -er '.parent_gid' "$sandbox_config")"
[[ "$parent_uid" =~ ^[0-9]+$ && "$parent_uid" != 0 \
  && "$parent_gid" =~ ^[0-9]+$ && "$parent_gid" != 0 ]] \
  || fail 'configured parent must be a non-root identity'
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
cargo_registry_cache_config="$(jq -er '.cargo_registry_cache' "$sandbox_config")"
cargo_registry_cache="$(realpath -e -- "$cargo_registry_cache_config")" \
  || fail 'Cargo registry cache missing'
[[ "$cargo_registry_cache" == "$cargo_registry_cache_config" ]] \
  || fail 'Cargo registry cache path contains a symlink or alias'
validate_cargo_registry_cache "$cargo_registry_cache"
grype_db_config="$(jq -er '.grype_db_root' "$sandbox_config")"
grype_db_root="$(realpath -e -- "$grype_db_config")" \
  || fail 'Grype database root missing'
[[ "$grype_db_root" == "$grype_db_config" ]] \
  || fail 'Grype database path contains a symlink or alias'
validate_grype_db "$grype_db_root" \
  "$(jq -er '.grype_db_inventory_sha256' "$sandbox_config")"
request_root="$(realpath -e -- "$(jq -er '.request_root' "$sandbox_config")")" \
  || fail 'root request directory missing'
[[ "$(stat -c '%u:%g:%a' -- "$request_root")" == '0:0:700' ]] \
  || fail 'root request directory ownership/mode mismatch'
request_root_options="$(/usr/bin/findmnt -rn -o OPTIONS --target "$request_root")" \
  || fail 'cannot inspect root request filesystem'
case ",$request_root_options," in
  *,noexec,*) fail 'root request filesystem forbids worker execution' ;;
esac
native_evidence_root="$(realpath -e -- \
  "$(jq -er '.native_evidence_root' "$sandbox_config")")" \
  || fail 'durable native evidence directory missing'
case "$native_evidence_root" in
  /tmp | /tmp/*) fail 'durable native evidence directory cannot use /tmp' ;;
esac
[[ ! -L "$native_evidence_root" \
  && "$(stat -c '%u:%g:%a' -- "$native_evidence_root")" \
  == '0:0:700' ]] \
  || fail 'durable native evidence ownership/mode mismatch'
proof_evidence_root="$(realpath -e -- \
  "$(jq -er '.proof_evidence_root' "$sandbox_config")")" \
  || fail 'durable proof evidence directory missing'
case "$proof_evidence_root" in
  /tmp | /tmp/*) fail 'durable proof evidence directory cannot use /tmp' ;;
esac
[[ "$proof_evidence_root" != "$native_evidence_root" \
  && ! -L "$proof_evidence_root" \
  && "$(stat -c '%u:%g:%a' -- "$proof_evidence_root")" == '0:0:700' ]] \
  || fail 'durable proof evidence ownership/mode mismatch'
case "$proof_evidence_root/" in
  "$native_evidence_root/"*) fail 'durable evidence directories cannot be nested' ;;
esac
case "$native_evidence_root/" in
  "$proof_evidence_root/"*) fail 'durable evidence directories cannot be nested' ;;
esac
systemd_version="$(systemctl show -p Version --value)" \
  || fail 'systemd manager unavailable'
[[ "$systemd_version" =~ ^[0-9]+([.][0-9]+)*([-+~][0-9A-Za-z][0-9A-Za-z.+:~_-]*)?$ ]] \
  || fail 'invalid systemd manager version'
command -v systemd-run >/dev/null || fail 'systemd-run unavailable'
for launcher in /usr/bin/unshare /usr/bin/setpriv /usr/bin/findmnt \
  /usr/bin/flock; do
  [[ ! -L "$launcher" \
    && "$(stat -c '%u:%a:%h' -- "$launcher" 2>/dev/null)" == '0:755:1' ]] \
    || fail "unsafe namespace launcher: $launcher"
done
for launcher in /usr/bin/mount /usr/bin/umount; do
  [[ ! -L "$launcher" \
    && "$(stat -c '%u:%a:%h' -- "$launcher" 2>/dev/null)" \
      =~ ^0:(755|4755):1$ ]] \
    || fail "unsafe quota mount tool: $launcher"
done
quota_probe="$(mktemp -d "$request_root/.quota-preflight.XXXXXX")" \
  || fail 'cannot create evidence quota probe'
cleanup_quota_probe() {
  /usr/bin/umount -- "$quota_probe" >/dev/null 2>&1 || true
  rmdir -- "$quota_probe" >/dev/null 2>&1 || true
}
trap cleanup_quota_probe EXIT
/usr/bin/mount -t tmpfs \
  -o 'nodev,nosuid,noexec,size=1048576,nr_inodes=16,mode=0700' \
  jain-host-ci-preflight "$quota_probe" \
  || fail 'bounded evidence tmpfs cannot be mounted'
[[ "$(/usr/bin/findmnt -rn -o FSTYPE,TARGET --target "$quota_probe")" \
  == "tmpfs $quota_probe" ]] \
  || fail 'bounded evidence tmpfs verification failed'
/usr/bin/umount -- "$quota_probe" || fail 'bounded evidence tmpfs cannot be unmounted'
rmdir -- "$quota_probe" || fail 'bounded evidence quota probe cleanup failed'
trap - EXIT
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
