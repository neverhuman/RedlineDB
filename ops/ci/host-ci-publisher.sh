#!/usr/bin/env bash
# Root-only one-shot publisher for a result sealed by host-ci-sandbox.
set -euo pipefail

if [[ "${JAIN_HOST_CI_CLEAN_ENV:-0}" != 1 ]]; then
  exec /usr/bin/env -i PATH=/usr/bin:/bin LC_ALL=C \
    JAIN_HOST_CI_CLEAN_ENV=1 /bin/bash "${BASH_SOURCE[0]}" "$@"
fi
export PATH=/usr/bin:/bin LC_ALL=C
unset CDPATH ENV BASH_ENV GIT_DIR GIT_WORK_TREE GIT_CONFIG_COUNT GIT_CONFIG
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
cd /

fail() {
  printf '[host-ci-publisher] %s\n' "$*" >&2
  exit 1
}

[[ "$(id -u)" == 0 ]] || fail 'must run as root'
[[ "$#" == 1 ]] || fail 'expected one root request directory'

publisher_path="$(realpath -e -- "${BASH_SOURCE[0]}")" \
  || fail 'cannot resolve publisher path'
install_dir="$(dirname "$publisher_path")"
config="$install_dir/host-ci-publisher.config.json"
[[ ! -L "$publisher_path" \
  && "$(stat -c '%u:%a:%h' -- "$publisher_path" 2>/dev/null)" == '0:500:1' ]] \
  || fail 'publisher must be root-owned mode 0500'
[[ ! -L "$install_dir" && -d "$install_dir" \
  && "$(stat -c '%u' -- "$install_dir")" == 0 \
  && "$((8#$(stat -c '%a' -- "$install_dir") & 8#022))" == 0 ]] \
  || fail 'publisher directory must be root-owned and write-protected'
[[ ! -L "$config" \
  && "$(stat -c '%u:%a:%h' -- "$config" 2>/dev/null)" == '0:600:1' ]] \
  || fail 'publisher config must be root-owned mode 0600'
jq -e '
  select(.schema_version == "jain.host-ci-publisher-config/v2")
  | select(.publisher_sha256 | test("^[0-9a-f]{64}$"))
  | select(.sandbox_sha256 | test("^[0-9a-f]{64}$"))
  | select(.forge_base | type == "string")
  | select(.forge_git_base | type == "string" and length > 0)
  | select(.control_remote | type == "string" and length > 0)
  | select(.request_root | type == "string" and startswith("/"))
  | select(.max_seal_age_seconds | type == "number" and . >= 1 and . <= 300)
  | select(.token | type == "string")' "$config" >/dev/null \
  || fail 'invalid publisher config schema'

publisher_sha="$(sha256sum -- "$publisher_path" | cut -d' ' -f1)"
expected_publisher_sha="$(jq -er '.publisher_sha256' "$config")"
[[ "$publisher_sha" == "$expected_publisher_sha" ]] \
  || fail 'publisher digest/config mismatch'
request_root="$(realpath -e -- "$(jq -er '.request_root' "$config")")" \
  || fail 'request root is unavailable'
[[ ! -L "$request_root" \
  && "$(stat -c '%u:%a' -- "$request_root")" == '0:700' ]] \
  || fail 'request root must be root-owned mode 0700'

request_dir="$(realpath -e -- "$1")" || fail 'request directory missing'
request_id="${request_dir##*/}"
[[ "$request_id" =~ ^[0-9a-f]{64}$ \
  && "$request_dir" == "$request_root/$request_id" \
  && ! -L "$request_dir" \
  && "$(stat -c '%u:%a' -- "$request_dir")" == '0:700' ]] \
  || fail 'request directory is outside root authority'
state="$request_dir/root-state.json"
result="$request_dir/root-result.json"
for artifact in "$state" "$result"; do
  [[ -f "$artifact" && ! -L "$artifact" \
    && "$(stat -c '%u:%a:%h' -- "$artifact")" == '0:600:1' ]] \
    || fail "unsafe root-sealed artifact: $artifact"
done

# The lock directory is the one-shot transition. It is created before reading
# any publication credential; a failed or crashed attempt cannot be replayed.
mkdir -m 0700 "$request_dir/publish.lock" 2>/dev/null \
  || fail 'request was already used or is being published'

jq -e --arg request_id "$request_id" \
  'select(.schema_version == "jain.host-ci-root-state/v2")
   | select(.request_id == $request_id and .status == "sealed")
   | select(.nonce | test("^[0-9a-f]{64}$"))
   | select(.result_sha256 | test("^[0-9a-f]{64}$"))
   | select(.root_seal | test("^[0-9a-f]{64}$"))
   | select(.sealed_at | type == "number")
   | select(.control_plane_commit | test("^[0-9a-f]{40}$"))
   | select(.publisher_sha256 | test("^[0-9a-f]{64}$"))
   | select(.sandbox_sha256 | test("^[0-9a-f]{64}$"))' "$state" >/dev/null \
  || fail 'root request is not sealed for one-shot publication'
nonce="$(jq -er '.nonce' "$state")"
result_sha="$(sha256sum -- "$result" | cut -d' ' -f1)"
[[ "$result_sha" == "$(jq -er '.result_sha256' "$state")" ]] \
  || fail 'root result digest mismatch'
sealed_at="$(jq -er '.sealed_at' "$state")"
now="$(date +%s)"
max_age="$(jq -er '.max_seal_age_seconds' "$config")"
age=$((now - sealed_at))
(( age >= 0 && age <= max_age )) || fail 'root request seal is stale'
expected_seal="$({
  printf '%s\n%s\n%s\n%s\n' "$nonce" "$result_sha" "$sealed_at" "$request_id"
} | sha256sum | cut -d' ' -f1)"
[[ "$expected_seal" == "$(jq -er '.root_seal' "$state")" ]] \
  || fail 'root result seal mismatch'
[[ "$(jq -er '.publisher_sha256' "$state")" == "$publisher_sha" \
  && "$(jq -er '.sandbox_sha256' "$state")" \
    == "$(jq -er '.sandbox_sha256' "$config")" ]] \
  || fail 'root request broker binding mismatch'

jq -e --arg request_id "$request_id" \
  'select(.schema_version == "jain.host-ci-root-result/v2")
   | select(.request_id == $request_id)
   | select(.control_plane_commit | test("^[0-9a-f]{40}$"))
   | select(.owner | test("^[a-z0-9][a-z0-9-]*$"))
   | select(.repository | test("^[a-z0-9][a-z0-9-]*$"))
   | select(.head_sha | test("^[0-9a-f]{40}$"))
   | select(.required_check | test("^[a-z0-9][a-z0-9-]*/required$"))
   | select(.conclusion == "success" or .conclusion == "failure")
   | select(.runner_exit_code | type == "number")
   | select(.native_evidence_required | type == "boolean")
   | select(.native_evidence_dir | type == "string")
   | select(.native_evidence_sha256 | type == "string")' "$result" >/dev/null \
  || fail 'invalid root result schema'

control_root="$request_dir/worker-authority/control-plane"
[[ -d "$control_root/.git" && ! -L "$control_root" \
  && "$(stat -c '%u' -- "$control_root")" == 0 ]] \
  || fail 'immutable control checkout missing'
if find "$control_root" -xdev \( -type f -o -type d \) \
  -perm /022 -print -quit | grep -q .; then
  fail 'immutable control checkout is group/world writable'
fi
for critical in repos.manifest.toml ops/ci/host-ci-publisher.sh \
  ops/ci/host-ci-sandbox.sh ops/ci/native-runtime.sh; do
  [[ -f "$control_root/$critical" && ! -L "$control_root/$critical" \
    && "$(stat -c '%u' -- "$control_root/$critical")" == 0 ]] \
    || fail "unsafe immutable control input: $critical"
done
control_commit="$(jq -er '.control_plane_commit' "$result")"
[[ "$control_commit" == "$(jq -er '.control_plane_commit' "$state")" ]] \
  || fail 'control commit differs across root artifacts'
safe_git=(git -c core.fsmonitor=false -c core.hooksPath=/dev/null \
  -c core.untrackedCache=false -c diff.external=)
[[ "$("${safe_git[@]}" -C "$control_root" rev-parse --verify 'HEAD^{commit}')" \
  == "$control_commit" ]] || fail 'immutable control checkout commit mismatch'
[[ "$(sha256sum -- "$control_root/ops/ci/host-ci-publisher.sh" | cut -d' ' -f1)" \
  == "$publisher_sha" ]] || fail 'publisher is not the reviewed authority byte'
[[ "$(sha256sum -- "$control_root/ops/ci/host-ci-sandbox.sh" | cut -d' ' -f1)" \
  == "$(jq -er '.sandbox_sha256' "$config")" ]] \
  || fail 'sandbox is not the reviewed authority byte'

manifest="$control_root/repos.manifest.toml"
repo="$(jq -er '.repository' "$result")"
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
  ' "$manifest"
} 2>/dev/null)" || fail 'repository authority is absent or ambiguous'
IFS=$'\t' read -r protected_owner protected_check <<<"$repo_authority"
owner="$(jq -er '.owner' "$result")"
required_check="$(jq -er '.required_check' "$result")"
[[ "$owner" == "$protected_owner" && "$required_check" == "$protected_check" ]] \
  || fail 'root result owner/check differs from manifest authority'

# Derive native evidence policy from the root-owned reviewed policy code. A
# worker-supplied boolean is never an authorization input.
# shellcheck source=ops/ci/native-runtime.sh
source "$control_root/ops/ci/native-runtime.sh"
derived_required=false
if jain_native_check_requires_evidence "$repo" "$required_check" "$protected_check"; then
  derived_required=true
fi
[[ "$(jq -er '.native_evidence_required' "$result")" == "$derived_required" ]] \
  || fail 'root result attempted a native evidence policy downgrade'
conclusion="$(jq -er '.conclusion' "$result")"
head_sha="$(jq -er '.head_sha' "$result")"
if [[ "$conclusion" == success ]]; then
  [[ "$(jq -er '.runner_exit_code' "$result")" == 0 ]] \
    || fail 'success result has a nonzero worker exit'
  evidence_dir="$(jq -er '.native_evidence_dir' "$result")"
  evidence_sha="$(jq -er '.native_evidence_sha256' "$result")"
  required_int=0
  [[ "$derived_required" == true ]] && required_int=1
  jain_verify_native_check_evidence success "$required_int" \
    "$evidence_dir" "$evidence_sha" "$head_sha" "$required_check" \
    "$control_root" "$control_commit" \
    || fail 'root result native evidence validation failed'
fi

control_remote="$(jq -er '.control_remote' "$config")"
[[ "$(jq -er '.control_remote' "$state")" == "$control_remote" ]] \
  || fail 'root request control remote binding mismatch'
if [[ "$conclusion" == success ]]; then
  reviewed_commit="$("${safe_git[@]}" ls-remote --exit-code \
    "$control_remote" refs/heads/main 2>/dev/null | cut -f1)" \
    || fail 'cannot read reviewed control-plane main'
  [[ "$reviewed_commit" == "$control_commit" ]] \
    || fail 'success authority is no longer reviewed main'
  forge_git_base="$(jq -er '.forge_git_base' "$config")"
  product_remote="${forge_git_base%/}/$owner/$repo.git"
  "${safe_git[@]}" ls-remote --exit-code "$product_remote" 2>/dev/null \
    | awk -v head="$head_sha" '$1 == head { found=1 } END { exit !found }' \
    || fail 'success head is not an advertised authoritative product ref'
fi

write_status() {
  local status="${1:?status required}" tmp="$request_dir/root-state.tmp.$$"
  jq --arg status "$status" '.status=$status' "$state" >"$tmp"
  chmod 0600 "$tmp"
  chown root:root "$tmp"
  mv -f -- "$tmp" "$state"
}
write_status publishing

# Only now, after one-shot transition and all root authority checks, read the
# credential. It remains in root memory and curl stdin, never argv/environment.
forge_base="$(jq -er '.forge_base' "$config")"
token="$(jq -er '.token' "$config")"
[[ "$forge_base" =~ ^http://127\.0\.0\.1:[0-9]{1,5}$ \
  && "$token" =~ ^[A-Za-z0-9._~+/-]{16,512}$ ]] \
  || fail 'invalid root publication endpoint or credential'
curl -fsS --max-time 5 "$forge_base/health" >/dev/null \
  || { write_status failed; fail 'forge is not healthy'; }
post_json() {
  local url="$1" payload="$2"
  printf 'header = "Authorization: Bearer %s"\n' "$token" \
    | curl --config - -fsS --max-time 10 -X POST "$url" \
      -H 'content-type: application/json' -d "$payload" >/dev/null
}
post_json "$forge_base/repos/$owner/$repo/check-runs" \
  "$(jq -cn --arg name "$required_check" --arg sha "$head_sha" \
    --arg conclusion "$conclusion" \
    '{name:$name,head_sha:$sha,status:"completed",conclusion:$conclusion}')" \
  || { write_status failed; fail 'check-run publication failed'; }
state_value=failure
[[ "$conclusion" == success ]] && state_value=success
description="$required_check root-seal=$(jq -er '.root_seal' "$state" | cut -c1-16)"
post_json "$forge_base/repos/$owner/$repo/statuses/$head_sha" \
  "$(jq -cn --arg state "$state_value" --arg context "$required_check" \
    --arg description "$description" \
    '{state:$state,context:$context,description:$description}')" \
  || { write_status failed; fail 'commit-status publication failed'; }
write_status consumed
printf '[host-ci-publisher] consumed one-shot %s for %s/%s @ %.12s\n' \
  "$request_id" "$owner" "$repo" "$head_sha" >&2
