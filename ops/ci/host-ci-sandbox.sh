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
[[ "$#" == 1 ]] || fail 'expected one sandbox request path'
request="$1"

sandbox_path="$(realpath -e -- "${BASH_SOURCE[0]}")" \
  || fail 'cannot resolve sandbox path'
install_dir="$(dirname "$sandbox_path")"
config="$install_dir/host-ci-sandbox.config.json"
publisher_path="$install_dir/host-ci-publisher"
splitctl_path="$install_dir/splitctl"
jankurai_path="$install_dir/jankurai"
security_tool_names=(actionlint grype syft)
[[ ! -L "$sandbox_path" \
  && "$(stat -c '%u:%a:%h' -- "$sandbox_path" 2>/dev/null)" == '0:500:1' ]] \
  || fail 'sandbox must be root-owned mode 0500'
[[ ! -L "$publisher_path" \
  && "$(stat -c '%u:%a:%h' -- "$publisher_path" 2>/dev/null)" == '0:500:1' ]] \
  || fail 'publisher must be root-owned mode 0500'
[[ ! -L "$splitctl_path" \
  && "$(stat -c '%u:%a:%h' -- "$splitctl_path" 2>/dev/null)" == '0:500:1' ]] \
  || fail 'splitctl must be root-owned mode 0500'
[[ ! -L "$jankurai_path" \
  && "$(stat -c '%u:%a:%h' -- "$jankurai_path" 2>/dev/null)" == '0:555:1' ]] \
  || fail 'Jankurai must be root-owned mode 0555'
[[ ! -L "$install_dir" && -d "$install_dir" \
  && "$(stat -c '%u' -- "$install_dir")" == 0 \
  && "$((8#$(stat -c '%a' -- "$install_dir") & 8#022))" == 0 ]] \
  || fail 'broker directory must be root-owned and write-protected'
[[ ! -L "$config" \
  && "$(stat -c '%u:%a:%h' -- "$config" 2>/dev/null)" == '0:600:1' ]] \
  || fail 'unsafe root sandbox config'
jq -e '
  select(.schema_version == "jain.host-ci-sandbox-config/v5")
  | select(.sandbox_sha256 | test("^[0-9a-f]{64}$"))
  | select(.publisher_sha256 | test("^[0-9a-f]{64}$"))
  | select(.splitctl_sha256 | test("^[0-9a-f]{64}$"))
  | select(.jankurai_sha256 | test("^[0-9a-f]{64}$"))
  | select((.security_tool_sha256 | keys) == ["actionlint", "grype", "syft"])
  | select(all(.security_tool_sha256[]; test("^[0-9a-f]{64}$")))
  | select(.parent_uid | type == "number")
  | select(.parent_gid | type == "number")
  | select(.worker_user | type == "string" and length > 0)
  | select(.worker_group | type == "string" and length > 0)
  | select(.family_root | type == "string" and startswith("/"))
  | select(.worker_cache | type == "string" and startswith("/"))
  | select(.cargo_registry_cache | type == "string" and startswith("/"))
  | select(.grype_db_root | type == "string" and startswith("/"))
  | select(.grype_db_inventory_sha256 | test("^[0-9a-f]{64}$"))
  | select(.cargo_bin | type == "string" and startswith("/"))
  | select(.rustup_home | type == "string" and startswith("/"))
  | select(.control_remote | type == "string" and length > 0)
  | select(.forge_git_base | type == "string" and length > 0)
  | select(.request_root | type == "string" and startswith("/"))
  | select(.native_evidence_root | type == "string" and startswith("/"))
  | select(.proof_evidence_root | type == "string" and startswith("/"))
  | select(.token_file | type == "string" and startswith("/"))
  | select(.retain_requests | type == "boolean")
  | select((.control_ref // "refs/heads/main") | type == "string")
  | select((.bootstrap_commit // "") | type == "string")
  | select((.bootstrap_expires_at // "") | type == "string")
  | select(.device_allow | type == "array")' "$config" >/dev/null \
  || fail 'invalid sandbox config schema'

sandbox_sha="$(sha256sum -- "$sandbox_path" | cut -d' ' -f1)"
publisher_sha="$(sha256sum -- "$publisher_path" | cut -d' ' -f1)"
splitctl_sha="$(sha256sum -- "$splitctl_path" | cut -d' ' -f1)"
jankurai_sha="$(sha256sum -- "$jankurai_path" | cut -d' ' -f1)"
[[ "$sandbox_sha" == "$(jq -er '.sandbox_sha256' "$config")" \
  && "$publisher_sha" == "$(jq -er '.publisher_sha256' "$config")" \
  && "$splitctl_sha" == "$(jq -er '.splitctl_sha256' "$config")" ]] \
  || fail 'installed broker digest/config mismatch'
[[ "$jankurai_sha" == "$(jq -er '.jankurai_sha256' "$config")" \
  && "$("$jankurai_path" --version)" == 'jankurai 1.6.11' ]] \
  || fail 'installed Jankurai digest/version mismatch'
security_tool_sha256="$(jq -c '.security_tool_sha256' "$config")"
for tool in "${security_tool_names[@]}"; do
  tool_path="$install_dir/security-$tool"
  [[ ! -L "$tool_path" \
    && "$(stat -c '%u:%g:%a:%h' -- "$tool_path" 2>/dev/null)" == '0:0:555:1' \
    && "$(sha256sum -- "$tool_path" | cut -d' ' -f1)" \
      == "$(jq -er --arg tool "$tool" '.security_tool_sha256[$tool]' "$config")" ]] \
    || fail "installed security tool digest/metadata mismatch: $tool"
done

parent_uid="$(jq -er '.parent_uid' "$config")"
parent_gid="$(jq -er '.parent_gid' "$config")"
[[ "$parent_uid" =~ ^[0-9]+$ && "$parent_uid" != 0 \
  && "$parent_gid" =~ ^[0-9]+$ && "$parent_gid" != 0 ]] \
  || fail 'parent must be a non-root identity'
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
cargo_registry_cache_config="$(jq -er '.cargo_registry_cache' "$config")"
cargo_registry_cache="$(realpath -e -- "$cargo_registry_cache_config")" \
  || fail 'Cargo registry cache unavailable'
[[ "$cargo_registry_cache" == "$cargo_registry_cache_config" ]] \
  || fail 'Cargo registry cache path contains a symlink or alias'
validate_cargo_registry_cache "$cargo_registry_cache"
grype_db_config="$(jq -er '.grype_db_root' "$config")"
grype_db_root="$(realpath -e -- "$grype_db_config")" \
  || fail 'Grype database root unavailable'
[[ "$grype_db_root" == "$grype_db_config" ]] \
  || fail 'Grype database path contains a symlink or alias'
grype_db_inventory_sha256="$(jq -er '.grype_db_inventory_sha256' "$config")"
validate_grype_db "$grype_db_root" "$grype_db_inventory_sha256"
cargo_bin="$(realpath -e -- "$(jq -er '.cargo_bin' "$config")")" \
  || fail 'Cargo bin directory unavailable'
rustup_home="$(realpath -e -- "$(jq -er '.rustup_home' "$config")")" \
  || fail 'rustup home unavailable'
request_root="$(realpath -e -- "$(jq -er '.request_root' "$config")")" \
  || fail 'root request directory unavailable'
request_root_options="$(/usr/bin/findmnt -rn -o OPTIONS --target "$request_root")" \
  || fail 'cannot inspect root request filesystem'
case ",$request_root_options," in
  *,noexec,*) fail 'root request filesystem forbids worker execution' ;;
esac
native_evidence_root="$(realpath -e -- \
  "$(jq -er '.native_evidence_root' "$config")")" \
  || fail 'durable native evidence directory unavailable'
proof_evidence_root="$(realpath -e -- \
  "$(jq -er '.proof_evidence_root' "$config")")" \
  || fail 'durable proof evidence directory unavailable'
token_file="$(jq -er '.token_file' "$config")"
[[ ! -L "$token_file" \
  && "$(realpath -e -- "$token_file" 2>/dev/null)" == "$token_file" \
  && "$(stat -c '%u:%g:%a:%h' -- "$token_file" 2>/dev/null)" == '0:0:600:1' ]] \
  || fail 'sandbox token file must be canonical root:root mode 0600 single-link'
[[ "$(stat -c '%u:%a' -- "$worker_cache")" == "$worker_uid:700" \
  && "$(stat -c '%u:%a' -- "$request_root")" == '0:700' \
  && ! -L "$native_evidence_root" \
  && "$(stat -c '%u:%g:%a' -- "$native_evidence_root")" \
    == '0:0:700' \
  && ! -L "$proof_evidence_root" \
  && "$(stat -c '%u:%g:%a' -- "$proof_evidence_root")" \
    == '0:0:700' \
  && "$proof_evidence_root" != "$native_evidence_root" ]] \
  || fail 'cache, request, or durable evidence ownership mismatch'
case "$proof_evidence_root/" in
  "$native_evidence_root/"*) fail 'durable evidence roots cannot be nested' ;;
esac
case "$native_evidence_root/" in
  "$proof_evidence_root/"*) fail 'durable evidence roots cannot be nested' ;;
esac
for evidence_root in "$native_evidence_root" "$proof_evidence_root"; do
  case "$evidence_root" in
    /tmp | /tmp/*) fail 'durable evidence root cannot use /tmp' ;;
  esac
done
for launcher in /usr/bin/unshare /usr/bin/setpriv /usr/bin/findmnt \
  /usr/bin/flock; do
  [[ ! -L "$launcher" \
    && "$(stat -c '%u:%a:%h' -- "$launcher")" == '0:755:1' ]] \
    || fail "unsafe namespace launcher: $launcher"
done
for launcher in /usr/bin/mount /usr/bin/umount; do
  [[ ! -L "$launcher" \
    && "$(stat -c '%u:%a:%h' -- "$launcher")" =~ ^0:(755|4755):1$ ]] \
    || fail "unsafe quota mount tool: $launcher"
done

caller_request="$(realpath -e -- "$request")" || fail 'sandbox request missing'
bootstrap_root="$(dirname "$caller_request")"
case "$bootstrap_root" in
  "$family_root"/target/host-ci-sandboxes/split-host-ci-bootstrap.??????) ;;
  *) fail 'request is outside a host-CI bootstrap directory' ;;
esac
[[ "$caller_request" == "$bootstrap_root/sandbox-request.json" \
  && -f "$caller_request" && ! -L "$caller_request" && ! -L "$bootstrap_root" \
  && "$(stat -c '%u:%g:%a:%h' -- "$caller_request")" \
    == "$parent_uid:$parent_gid:600:1" \
  && "$(stat -c '%u:%g:%a' -- "$bootstrap_root")" \
    == "$parent_uid:$parent_gid:700" \
  && "$(stat -c '%s' -- "$caller_request")" -le 65536 ]] \
  || fail 'unsafe sandbox request ownership or location'

# Snapshot caller bytes into root-only storage before parsing. The caller may
# retain a writable descriptor across the later bootstrap chown, but that
# descriptor can never alter this single authority input.
request_id="$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')"
nonce="$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')"
[[ "$request_id" =~ ^[0-9a-f]{64}$ && "$nonce" =~ ^[0-9a-f]{64}$ ]] \
  || fail 'root randomness unavailable'
root_request="$request_root/$request_id"
mkdir -m 0700 "$root_request" || fail 'cannot create root request'
retain_requests="$(jq -r '.retain_requests' "$config")"
[[ "$retain_requests" == true || "$retain_requests" == false ]] \
  || fail 'retain_requests is not a validated boolean'
cleanup_root_request() {
  if [[ "$retain_requests" != true ]]; then
    rm -rf -- "$root_request"
  fi
}
trap cleanup_root_request EXIT
request="$root_request/caller-request.json"
"$splitctl_path" host-ci-snapshot-request \
  --source "$caller_request" --destination "$request" \
  --expected-uid "$parent_uid" --expected-gid "$parent_gid" \
  --max-bytes 65536 \
  || fail 'cannot snapshot caller request'
[[ ! -L "$request" \
  && "$(stat -c '%u:%g:%a:%h' -- "$request")" == '0:0:600:1' \
  && "$(stat -c '%s' -- "$request")" -le 65536 ]] \
  || fail 'unsafe root request snapshot'
jq -e '
  select(.schema_version == "jain.host-ci-sandbox-request/v4")
  | select(.control_plane_commit | test("^[0-9a-f]{40}$"))
  | select(.split_root | type == "string" and startswith("/"))
  | select(.arguments | type == "array" and length == 5)
  | select(.environment | type == "object")
  | select(.environment | all(to_entries[]; .value | type == "string"))' \
  "$request" >/dev/null || fail 'invalid sandbox request schema'
control_commit="$(jq -er '.control_plane_commit' "$request")"
split_root="$(realpath -e -- "$(jq -er '.split_root' "$request")")" \
  || fail 'split root unavailable'
[[ "$split_root" == "$family_root" ]] || fail 'split root is not configured authority'
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
    && "$name" != JAIN_NATIVE_EVIDENCE_ROOT \
    && "$name" != JAIN_NATIVE_EVIDENCE_STAGING_ROOT \
    && "$name" != JAIN_PROOF_EVIDENCE_ROOT \
    && "$name" != JAIN_PROOF_EVIDENCE_STAGING_ROOT \
    && "$name" != JAIN_RUSTSEC_ADVISORY_SOURCE \
    && "$name" != JAIN_GRYPE_DB_ROOT \
    && "$name" != JAIN_GRYPE_DB_INVENTORY_SHA256 \
    && "$name" != JAIN_SPLIT_OPS_ROOT \
    && "$name" != JAIN_HOST_CI_REEXEC_STATE \
    && "$name" != JAIN_HOST_CI_NETWORK_ISOLATED \
    && "$name" != JAIN_HOST_CI_HOST_PID_NAMESPACE \
    && "$name" != JAIN_HOST_CI_HOST_USER_NAMESPACE ]] \
    || fail "forbidden sandbox environment key: $name"
done
[[ "$(jq -er '.environment.CARGO_TARGET_DIR' "$request")" \
  == "$bootstrap_root/cargo-target" \
  && "$(jq -er '.environment.JAIN_HOST_CI_WRITABLE_ROOT' "$request")" \
    == "$bootstrap_root/writable" \
  && "$(jq -er '.environment.JAIN_SPLIT_ROOT' "$request")" == "$family_root" \
  && "$(jq -er '.environment.JAIN_RELEASE_CI' "$request")" == 1 ]] \
  || fail 'worker paths or mandatory release mode differ from root authority'

worker_authority="$root_request/worker-authority"
control_root="$worker_authority/control-plane"
mkdir -m 0755 "$worker_authority"

safe_git=(git -c core.fsmonitor=false -c core.hooksPath=/dev/null \
  -c core.untrackedCache=false -c diff.external=)
control_remote="$(jq -er '.control_remote' "$config")"
control_ref="$(jq -er '.control_ref // "refs/heads/main"' "$config")"
bootstrap_commit="$(jq -er '.bootstrap_commit // ""' "$config")"
bootstrap_expires_at="$(jq -er '.bootstrap_expires_at // ""' "$config")"
control_repository=veox/jain-split-ops
validate_control_authority \
  "$control_ref" "$bootstrap_commit" "$bootstrap_expires_at"
if [[ "$control_ref" != refs/heads/main && "$bootstrap_commit" != "$control_commit" ]]; then
  fail 'bootstrap control commit differs from the exact request'
fi
control_authority_json="$("$splitctl_path" jeryu-local authority-readback \
  --repo "$control_repository" --remote "$control_remote" \
  --ref "$control_ref" --expected-head "$control_commit" \
  --token-file "$token_file")" \
  || fail 'cannot authenticate configured control authority'
jq -e --arg repository "$control_repository" --arg remote "$control_remote" \
  --arg ref "$control_ref" --arg commit "$control_commit" '
  select(.schema_version == "jain.jeryu-authority-readback/v1")
  | select(.repository == $repository and .remote == $remote)
  | select(.ref == $ref and .commit == $commit)
  | select(.api_identity.host == "jeryu")
  | select(.api_identity.owner + "/" + .api_identity.name == $repository)
  | select(.api_identity.default_branch == "main")
  | select(.api_identity.clone_http_url == ("/git/" + $repository + ".git"))' \
  <<<"$control_authority_json" >/dev/null \
  || fail 'invalid authenticated control authority readback'
control_api_identity="$(jq -c '.api_identity' <<<"$control_authority_json")" \
  || fail 'cannot normalize authenticated control API identity'
control_materialization_json="$("$splitctl_path" jeryu-local git-materialize \
  --repo "$control_repository" --remote "$control_remote" \
  --ref "$control_ref" --expected-head "$control_commit" \
  --destination "$control_root" --token-file "$token_file" \
  --retain-origin)" \
  || fail 'cannot materialize authenticated configured control authority'
jq -e --arg repository "$control_repository" --arg remote "$control_remote" \
  --arg ref "$control_ref" --arg commit "$control_commit" \
  --argjson api_identity "$control_api_identity" '
  select(.schema_version == "jain.jeryu-git-materialization/v1")
  | select(.repository == $repository and .remote == $remote)
  | select(.reference == $ref and .commit == $commit)
  | select(.api_identity == $api_identity)' \
  <<<"$control_materialization_json" >/dev/null \
  || fail 'materialized control authority differs from authenticated readback'
[[ "$("${safe_git[@]}" -C "$control_root" rev-parse 'HEAD^{commit}')" \
  == "$control_commit" ]] || fail 'root immutable checkout mismatch'
[[ "$(sha256sum -- "$control_root/ops/ci/host-ci-sandbox.sh" | cut -d' ' -f1)" \
  == "$sandbox_sha" \
  && "$(sha256sum -- "$control_root/ops/ci/host-ci-publisher.sh" | cut -d' ' -f1)" \
    == "$publisher_sha" ]] \
  || fail 'installed brokers do not match reviewed main'

# Materialize the exact reviewed RustSec commit while still root. The canonical
# host source may be a linked worktree whose private Git metadata is deliberately
# invisible to the worker; only this standalone, read-only snapshot crosses the
# privilege boundary.
rustsec_source="$family_root/target/advisory-db"
rustsec_source="$(realpath -e -- "$rustsec_source")" \
  || fail 'canonical pinned RustSec source is unavailable'
# shellcheck source=ops/ci/pinned-advisory.sh
source "$control_root/ops/ci/pinned-advisory.sh"
rustsec_stage="$worker_authority/advisory-db"
jain_materialize_pinned_advisory_db \
  "$rustsec_source" "$rustsec_stage" "$JAIN_PINNED_RUSTSEC_COMMIT" \
  "$parent_uid" "$parent_gid" \
  || fail 'root pinned RustSec staging failed'
[[ -d "$rustsec_stage/.git" && ! -L "$rustsec_stage/.git" \
  && "$(git -c core.fsmonitor=false -c core.hooksPath=/dev/null \
    -C "$rustsec_stage" rev-parse 'HEAD^{commit}')" \
    == "$JAIN_PINNED_RUSTSEC_COMMIT" ]] \
  || fail 'root pinned RustSec snapshot is not standalone authority'

# Resolve owner/check only from the root-fetched manifest.
repo="${arguments[1]}"
repo_authority_json="$("$splitctl_path" host-ci-authority \
  --manifest "$control_root/repos.manifest.toml" --repo "$repo")" \
  || fail 'repository authority is absent or ambiguous'
repo_authority="$(jq -er --arg repo "$repo" '
  select(.schema_version == "jain.host-ci-repository-authority/v1")
  | select(.repository == $repo)
  | select(.forge_owner | test("^[A-Za-z0-9._-]+$"))
  | select(.required_check == ($repo + "/required"))
  | select(.remote | type == "string")
  | [.forge_owner, .required_check] | @tsv
' <<<"$repo_authority_json")" || fail 'invalid repository authority result'
IFS=$'\t' read -r protected_owner protected_check <<<"$repo_authority"
[[ "${arguments[0]}" == "$protected_owner" \
  && "${arguments[4]}" == "$protected_check" ]] \
  || fail 'requested owner/check differs from manifest authority'

# Never run a caller-supplied product checkout. Resolve an advertised ref from
# the configured forge Git root into root-owned storage, then stage an
# independent detached clone for the worker before changing bootstrap ownership.
forge_git_base="$(jq -er '.forge_git_base' "$config")"
product_remote="${forge_git_base%/}/$protected_owner/$repo.git"
product_authority="$root_request/product-authority"
"$splitctl_path" jeryu-local git-materialize \
  --repo "$protected_owner/$repo" --remote "$product_remote" \
  --expected-head "${arguments[2]}" --destination "$product_authority" \
  --token-file "$token_file" >/dev/null \
  || fail 'cannot materialize authenticated product authority'
[[ "$("${safe_git[@]}" -C "$product_authority" rev-parse 'HEAD^{commit}')" \
  == "${arguments[2]}" ]] || fail 'root product checkout commit mismatch'
git clone --quiet --no-local --no-checkout \
  "$product_authority" "${arguments[3]}"
git -C "${arguments[3]}" checkout --quiet --detach "${arguments[2]}"

# The proof auditor never reuses the product worker's mutable checkout. Root
# creates a second standalone exact-head checkout, strips its remote, verifies
# a clean tracked tree, and exposes it read-only only after the product cgroup
# has been killed.
audit_source_root="$root_request/audit-source"
audit_worktree="$audit_source_root/$repo"
mkdir -m 0755 "$audit_source_root"
git clone --quiet --no-local --no-checkout "$product_authority" "$audit_worktree"
git -C "$audit_worktree" checkout --quiet --detach "${arguments[2]}"
git -C "$audit_worktree" remote remove origin
[[ "$(git -c core.fsmonitor=false -c core.hooksPath=/dev/null \
    -c diff.external= -C "$audit_worktree" rev-parse 'HEAD^{commit}')" \
    == "${arguments[2]}" \
  && -z "$(git -c core.fsmonitor=false -c core.hooksPath=/dev/null \
    -c diff.external= -C "$audit_worktree" status --porcelain=v1 \
      --untracked-files=all)" ]] \
  || fail 'root exact-head audit checkout is not clean authority'
chmod -R go-w "$audit_source_root"
chown -R root:root "$audit_source_root"

install -o root -g root -m 0555 "$splitctl_path" "$worker_authority/splitctl"
install -d -o root -g root -m 0555 "$worker_authority/security-bin"
for tool in "${security_tool_names[@]}"; do
  install -o root -g root -m 0555 \
    "$install_dir/security-$tool" "$worker_authority/security-bin/$tool"
done
install -d -o root -g root -m 0555 "$worker_authority/release-bin"
install -o root -g root -m 0555 \
  "$jankurai_path" "$worker_authority/release-bin/jankurai"
install -o root -g root -m 0555 \
  "$control_root/ops/ci/split-host-ci.sh" \
  "$worker_authority/.split-host-ci-reviewed"
worker_result="$bootstrap_root/writable/worker-evidence.json"
jq -n --arg control_repository "$control_repository" \
  --arg control_remote "$control_remote" \
  --argjson control_api_identity "$control_api_identity" \
  --arg control_ref "$control_ref" --arg commit "$control_commit" \
  --arg result "$worker_result" \
  '{schema_version:"jain.host-ci-reexec/v5",
    source_root:"/opt/jain-ci/authority/control-plane",
    exact_root:"/opt/jain-ci/authority/control-plane",
    control_repository:$control_repository,control_remote:$control_remote,
    control_api_identity:$control_api_identity,control_ref:$control_ref,
    control_plane_commit:$commit,result_path:$result,
    splitctl_path:"/opt/jain-ci/authority/splitctl"}' \
  >"$worker_authority/reexec-state.json"
chmod 0444 "$worker_authority/reexec-state.json"
chown root:root "$worker_authority/reexec-state.json"
# The root materializer deliberately creates a 0700 staging root. After every
# identity check is complete, expose the reviewed control checkout read-only to
# the worker with the same 0755/0644-or-executable shape as the former clone.
chmod -R a+rX,go-w "$control_root"
chown -R root:root "$worker_authority"

created_at="$(date +%s)"
root_state="$root_request/root-state.json"
jq -n --arg request_id "$request_id" --arg nonce "$nonce" \
  --arg control_repository "$control_repository" \
  --arg commit "$control_commit" --arg remote "$control_remote" \
  --argjson control_api_identity "$control_api_identity" \
  --arg control_ref "$control_ref" \
  --arg bootstrap_expires_at "$bootstrap_expires_at" \
  --arg publisher_sha "$publisher_sha" --arg sandbox_sha "$sandbox_sha" \
  --arg splitctl_sha "$splitctl_sha" \
  --arg jankurai_sha "$jankurai_sha" \
  --argjson security_tool_sha256 "$security_tool_sha256" \
  --arg grype_db_root "$grype_db_root" \
  --arg grype_db_inventory_sha256 "$grype_db_inventory_sha256" \
  --arg native_evidence_root "$native_evidence_root" \
  --arg proof_evidence_root "$proof_evidence_root" \
  --argjson created_at "$created_at" \
  '{schema_version:"jain.host-ci-root-state/v5",status:"running",
    request_id:$request_id,nonce:$nonce,created_at:$created_at,
    control_repository:$control_repository,control_remote:$remote,
    control_api_identity:$control_api_identity,control_ref:$control_ref,
    control_plane_commit:$commit,
    bootstrap_expires_at:$bootstrap_expires_at,
    publisher_sha256:$publisher_sha,sandbox_sha256:$sandbox_sha,
    splitctl_sha256:$splitctl_sha,jankurai_sha256:$jankurai_sha,
    security_tool_sha256:$security_tool_sha256,
    grype_db_root:$grype_db_root,
    grype_db_inventory_sha256:$grype_db_inventory_sha256,
    native_evidence_root:$native_evidence_root,
    proof_evidence_root:$proof_evidence_root}' >"$root_state"
chmod 0600 "$root_state"
chown root:root "$root_state"

evidence_staging_root="$bootstrap_root/writable/native-evidence-staging"
mkdir -m 0700 "$evidence_staging_root"
/usr/bin/mount -t tmpfs \
  -o "nodev,nosuid,noexec,size=33554432,nr_inodes=64,mode=0700,uid=$worker_uid,gid=$worker_gid" \
  "jain-host-ci-evidence-$request_id" "$evidence_staging_root" \
  || fail 'cannot mount bounded native evidence staging'
evidence_mounted=1
[[ "$(/usr/bin/findmnt -rn -o FSTYPE,TARGET --target "$evidence_staging_root")" \
  == "tmpfs $evidence_staging_root" ]] \
  || fail 'native evidence staging is not the expected tmpfs'

unit="jain-host-ci-${request_id:0:24}.service"
audit_unit="jain-host-ci-proof-${request_id:0:18}.service"
proof_staging_root="$root_request/proof-staging"
proof_mounted=0
cleanup_evidence_mount() {
  if [[ "${evidence_mounted:-0}" == 1 ]]; then
    /usr/bin/umount -- "$evidence_staging_root" >/dev/null 2>&1 || true
    evidence_mounted=0
  fi
}
cleanup_proof_mount() {
  if [[ "${proof_mounted:-0}" == 1 ]]; then
    /usr/bin/umount -- "$proof_staging_root" >/dev/null 2>&1 || true
    proof_mounted=0
  fi
}
restore_owner() {
  systemctl kill --kill-whom=all --signal=KILL "$unit" >/dev/null 2>&1 || true
  systemctl kill --kill-whom=all --signal=KILL "$audit_unit" \
    >/dev/null 2>&1 || true
  systemctl reset-failed "$unit" >/dev/null 2>&1 || true
  systemctl reset-failed "$audit_unit" >/dev/null 2>&1 || true
  cleanup_evidence_mount
  cleanup_proof_mount
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
  # PrivateNetwork keeps the worker disconnected from the host and forge while
  # AF_INET/AF_INET6 allow real loopback HTTP fixtures inside that namespace.
  --property='RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6'
  --property=DevicePolicy=closed --property=PrivateDevices=no
  --property=KillMode=control-group --property=SendSIGKILL=yes
  --property=TimeoutStopSec=5s
  --property="BindPaths=$bootstrap_root"
  --property="BindPaths=$worker_cache:/opt/jain-ci/cargo-home"
  --property="BindReadOnlyPaths=$cargo_registry_cache:/opt/jain-ci/cargo-registry"
  --property="BindReadOnlyPaths=$grype_db_root:/opt/jain-ci/grype-db"
  --property="BindReadOnlyPaths=$worker_authority:/opt/jain-ci/authority"
  --property="BindReadOnlyPaths=$family_root"
  --property="BindReadOnlyPaths=$cargo_bin:/opt/jain-ci/cargo-bin"
  --property="BindReadOnlyPaths=$rustup_home:/opt/jain-ci/rustup"
  --property="BindPaths=$evidence_staging_root"
  --property="InaccessiblePaths=/usr/bin/sudo /etc/sudoers /etc/sudoers.d -$install_dir -$request_root"
  --setenv="HOME=$bootstrap_root/child-home"
  --setenv="USER=$worker_user" --setenv="LOGNAME=$worker_user"
  --setenv=SHELL=/bin/bash
  --setenv=PATH=/opt/jain-ci/authority/security-bin:/opt/jain-ci/authority/release-bin:/opt/jain-ci/cargo-bin:/usr/bin:/bin
  --setenv=CARGO_HOME=/opt/jain-ci/cargo-home
  --setenv=RUSTUP_HOME=/opt/jain-ci/rustup
  --setenv=JAIN_HOST_CI_REEXEC_STATE=/opt/jain-ci/authority/reexec-state.json
  --setenv=JAIN_SPLIT_OPS_ROOT=/opt/jain-ci/authority/control-plane
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
  --setenv=JAIN_RUSTSEC_ADVISORY_SOURCE=/opt/jain-ci/authority/advisory-db
  --setenv=JAIN_GRYPE_DB_ROOT=/opt/jain-ci/grype-db
  --setenv="JAIN_GRYPE_DB_INVENTORY_SHA256=$grype_db_inventory_sha256"
  --setenv=GRYPE_DB_CACHE_DIR=/opt/jain-ci/grype-db
  --setenv="JAIN_NATIVE_EVIDENCE_STAGING_ROOT=$evidence_staging_root"
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

# The auditor is a separately installed, digest-pinned trust input. It runs
# only after every product process is dead, in its own private network and PID
# namespace, against the separate root-owned exact-head checkout. Source is
# read-only; the only writable mount is a bounded output tmpfs.
mkdir -m 0700 "$proof_staging_root"
/usr/bin/mount -t tmpfs \
  -o "nodev,nosuid,noexec,size=16777216,nr_inodes=32,mode=0700,uid=$worker_uid,gid=$worker_gid" \
  "jain-host-ci-proof-$request_id" "$proof_staging_root" \
  || fail 'cannot mount bounded proof evidence staging'
proof_mounted=1
[[ "$(/usr/bin/findmnt -rn -o FSTYPE,TARGET --target "$proof_staging_root")" \
  == "tmpfs $proof_staging_root" ]] \
  || fail 'proof evidence staging is not the expected tmpfs'

audit_clean_start=false
if [[ -z "$(git -c core.fsmonitor=false -c core.hooksPath=/dev/null \
    -c diff.external= -C "$audit_worktree" status --porcelain=v1 \
      --untracked-files=all)" ]]; then
  audit_clean_start=true
fi
[[ "$audit_clean_start" == true ]] || fail 'exact-head audit checkout became dirty'

audit_systemd_args=(
  --quiet --wait --pipe --collect --service-type=exec --unit="$audit_unit"
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
  --property=DevicePolicy=closed --property=PrivateDevices=yes
  --property=KillMode=control-group --property=SendSIGKILL=yes
  --property=TimeoutStopSec=5s
  --property="BindReadOnlyPaths=$audit_worktree:/opt/jain-ci/repository"
  --property="BindReadOnlyPaths=$jankurai_path:/opt/jain-ci/jankurai"
  --property="BindPaths=$proof_staging_root:/opt/jain-ci/output"
  --property="InaccessiblePaths=/usr/bin/sudo /etc/sudoers /etc/sudoers.d -$install_dir -$request_root"
  --setenv=HOME=/tmp --setenv="USER=$worker_user" --setenv="LOGNAME=$worker_user"
  --setenv=SHELL=/bin/bash --setenv=PATH=/usr/bin:/bin
  --setenv=GIT_CONFIG_GLOBAL=/dev/null --setenv=GIT_CONFIG_NOSYSTEM=1
  --setenv=GIT_CONFIG_COUNT=1 --setenv=GIT_CONFIG_KEY_0=safe.directory
  --setenv=GIT_CONFIG_VALUE_0=/opt/jain-ci/repository
  --setenv=GIT_TERMINAL_PROMPT=0 --setenv=JANKURAI_NO_UPDATE_CHECK=1
  --setenv="JAIN_HOST_CI_HOST_PID_NAMESPACE=$host_pid_namespace"
  --setenv="JAIN_HOST_CI_HOST_USER_NAMESPACE=$host_user_namespace"
)
audit_rc=0
systemd-run "${audit_systemd_args[@]}" \
  /usr/bin/unshare --pid --fork --kill-child=KILL --mount-proc \
  /usr/bin/setpriv --inh-caps=-all --ambient-caps=-all \
    --bounding-set=-all --no-new-privs \
    /bin/bash -ceu '
      [[ "$(readlink /proc/self/ns/pid)" \
        != "${JAIN_HOST_CI_HOST_PID_NAMESPACE:?}" ]]
      [[ "$(readlink /proc/self/ns/user)" \
        != "${JAIN_HOST_CI_HOST_USER_NAMESPACE:?}" ]]
      unset JAIN_HOST_CI_HOST_PID_NAMESPACE JAIN_HOST_CI_HOST_USER_NAMESPACE
      umask 077
      cd /opt/jain-ci/repository
      exec /opt/jain-ci/jankurai audit . \
        --full --mode advisory --policy agent/audit-policy.toml \
        --json /opt/jain-ci/output/report.json \
        --md /opt/jain-ci/output/report.md \
        --repair-queue-jsonl /opt/jain-ci/output/repair-queue.jsonl \
        --no-score-history
    ' || audit_rc=$?
if [[ "$audit_rc" != 0 ]]; then
  journalctl --quiet --no-pager --unit "$audit_unit" --lines=80 >&2 || true
fi
systemctl kill --kill-whom=all --signal=KILL "$audit_unit" >/dev/null 2>&1 || true
systemctl is-active --quiet "$audit_unit" \
  && fail 'proof auditor cgroup remained active after exit'
printf '[host-ci-sandbox] proof auditor cgroup stopped before validation\n' >&2

conclusion=failure
evidence_dir=''
evidence_sha=''
promoted_evidence_dir=''
if [[ "$runner_rc" == 0 && -f "$worker_result" && ! -L "$worker_result" \
  && "$(stat -c '%u:%a:%h' -- "$worker_result")" == "$worker_uid:600:1" ]] \
  && jq -e --arg owner "${arguments[0]}" --arg repo "$repo" \
    --arg head "${arguments[2]}" --arg check "${arguments[4]}" \
    --arg control_repository "$control_repository" \
    --arg control_remote "$control_remote" \
    --argjson control_api_identity "$control_api_identity" \
    --arg control_ref "$control_ref" --arg commit "$control_commit" \
    'select(.schema_version == "jain.host-ci-worker-evidence/v5")
     | select(.owner == $owner and .repository == $repo)
     | select(.head_sha == $head and .required_check == $check)
     | select(.control_repository == $control_repository)
     | select(.control_remote == $control_remote)
     | select(.control_api_identity == $control_api_identity)
     | select(.control_ref == $control_ref)
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
# shellcheck source=ops/ci/host-ci-evidence.sh
source "$control_root/ops/ci/host-ci-evidence.sh"
derived_required=false
if jain_native_check_requires_evidence "$repo" "${arguments[4]}" "$protected_check"; then
  derived_required=true
fi
if [[ "$conclusion" == success ]]; then
  if [[ "$derived_required" == true ]]; then
    promoted_evidence_dir="$(jain_host_ci_promote_native_evidence \
      "$evidence_staging_root" "$evidence_dir" "$evidence_sha" \
      "$native_evidence_root" "${arguments[0]}" "$repo" "${arguments[2]}" \
      "${arguments[4]}" "$request_id" "$worker_uid" "$worker_gid" \
      "$control_root" "$control_commit")" || {
      printf '[host-ci-sandbox] bounded native evidence promotion failed\n' >&2
      conclusion=failure
    }
    if [[ "$conclusion" == success ]]; then
      evidence_dir="$promoted_evidence_dir"
    fi
  elif [[ -n "$evidence_dir" || -n "$evidence_sha" ]] \
    || ! jain_host_ci_staging_is_empty "$evidence_staging_root"; then
    printf '[host-ci-sandbox] non-native worker wrote unexpected evidence\n' >&2
    conclusion=failure
  fi
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

# The proof receipt is mandatory for every forge publication attempt. A failed
# product/native lane may produce a validated failure receipt, but malformed,
# forged, linked, oversized, or identity-mismatched auditor output produces no
# publication authority at all.
# shellcheck source=ops/ci/host-ci-proof-evidence.sh
source "$control_root/ops/ci/host-ci-proof-evidence.sh"
lane_conclusion=success
lane_failure_reason=''
if [[ "$conclusion" != success ]]; then
  lane_conclusion=failure
  lane_failure_reason="product/native lane failed (runner_exit_code=$runner_rc)"
elif [[ "$audit_rc" != 0 ]]; then
  lane_conclusion=failure
  lane_failure_reason="Jankurai auditor exited nonzero (exit_code=$audit_rc)"
fi
proof_attempt_id="$request_id"
proof_result_line="$(jain_host_ci_promote_proof_evidence \
  "$proof_staging_root" "$proof_evidence_root" \
  "${arguments[0]}" "$repo" "${arguments[2]}" "${arguments[4]}" \
  "$request_id" "$proof_attempt_id" "$worker_uid" "$worker_gid" \
  "$audit_worktree" "$splitctl_path" "$jankurai_path" "$jankurai_sha" \
  "$lane_conclusion" "$lane_failure_reason" "$audit_clean_start")" \
  || fail 'root exact-SHA proof evidence promotion failed'
IFS=$'\t' read -r proof_evidence_dir proof_receipt_sha proof_report_sha \
  proof_status proof_validator_rc <<<"$proof_result_line"
[[ -n "$proof_evidence_dir" && "$proof_receipt_sha" =~ ^[0-9a-f]{64}$ \
  && "$proof_report_sha" =~ ^[0-9a-f]{64}$ \
  && "$proof_status" =~ ^(pass|fail)$ \
  && "$proof_validator_rc" =~ ^[0-9]+$ ]] \
  || fail 'root exact-SHA proof evidence result is malformed'
if [[ "$proof_status" != pass || "$audit_rc" != 0 ]]; then
  conclusion=failure
fi

/usr/bin/umount -- "$proof_staging_root" \
  || fail 'cannot unmount bounded proof evidence staging'
proof_mounted=0
if /usr/bin/findmnt -rn -M "$proof_staging_root" >/dev/null; then
  fail 'proof evidence staging mount survived root validation'
fi

/usr/bin/umount -- "$evidence_staging_root" \
  || fail 'cannot unmount bounded native evidence staging'
evidence_mounted=0
if /usr/bin/findmnt -rn -M "$evidence_staging_root" >/dev/null; then
  fail 'native evidence staging mount survived worker validation'
fi

root_result="$root_request/root-result.json"
jq -n --arg request_id "$request_id" \
  --arg control_repository "$control_repository" \
  --arg control_remote "$control_remote" \
  --argjson control_api_identity "$control_api_identity" \
  --arg control_ref "$control_ref" --arg commit "$control_commit" \
  --arg owner "${arguments[0]}" --arg repo "$repo" \
  --arg head "${arguments[2]}" --arg check "${arguments[4]}" \
  --arg conclusion "$conclusion" --arg evidence_dir "$evidence_dir" \
  --arg evidence_sha "$evidence_sha" --argjson rc "$runner_rc" \
  --arg proof_dir "$proof_evidence_dir" \
  --arg proof_receipt "$proof_evidence_dir/receipt.json" \
  --arg proof_receipt_sha "$proof_receipt_sha" \
  --arg proof_report "$proof_evidence_dir/report.json" \
  --arg proof_report_sha "$proof_report_sha" \
  --arg proof_status "$proof_status" --arg proof_attempt "$proof_attempt_id" \
  --argjson audit_rc "$audit_rc" \
  --argjson proof_validator_rc "$proof_validator_rc" \
  --argjson evidence_required "$derived_required" \
  '{schema_version:"jain.host-ci-root-result/v5",request_id:$request_id,
    control_repository:$control_repository,control_remote:$control_remote,
    control_api_identity:$control_api_identity,control_ref:$control_ref,
    control_plane_commit:$commit,owner:$owner,repository:$repo,head_sha:$head,
    required_check:$check,conclusion:$conclusion,runner_exit_code:$rc,
    native_evidence_required:$evidence_required,
    native_evidence_dir:$evidence_dir,native_evidence_sha256:$evidence_sha,
    proof_evidence_required:true,proof_evidence_dir:$proof_dir,
    proof_receipt_path:$proof_receipt,
    proof_receipt_sha256:$proof_receipt_sha,
    proof_report_path:$proof_report,proof_report_sha256:$proof_report_sha,
    proof_status:$proof_status,proof_attempt_id:$proof_attempt,
    proof_auditor_exit_code:$audit_rc,
    proof_validator_exit_code:$proof_validator_rc}' \
  >"$root_result"
chmod 0600 "$root_result"
chown root:root "$root_result"
result_sha="$(sha256sum -- "$root_result" | cut -d' ' -f1)"
sealed_at="$(date +%s)"
root_seal="$({
  printf '%s\n%s\n%s\n%s\n%s\n' \
    "$nonce" "$result_sha" "$proof_receipt_sha" "$sealed_at" "$request_id"
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
