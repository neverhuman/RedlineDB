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

[[ "$(id -u)" == 0 ]] || fail 'must run as root'
[[ "$#" == 1 ]] || fail 'expected one root request directory'

publisher_path="$(realpath -e -- "${BASH_SOURCE[0]}")" \
  || fail 'cannot resolve publisher path'
install_dir="$(dirname "$publisher_path")"
config="$install_dir/host-ci-publisher.config.json"
splitctl_path="$install_dir/splitctl"
jankurai_path="$install_dir/jankurai"
security_tool_names=(actionlint grype syft)
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
[[ ! -L "$splitctl_path" \
  && "$(stat -c '%u:%a:%h' -- "$splitctl_path" 2>/dev/null)" == '0:500:1' ]] \
  || fail 'splitctl must be root-owned mode 0500'
[[ ! -L "$jankurai_path" \
  && "$(stat -c '%u:%a:%h' -- "$jankurai_path" 2>/dev/null)" == '0:555:1' ]] \
  || fail 'Jankurai must be root-owned mode 0555'
jq -e '
  select(.schema_version == "jain.host-ci-publisher-config/v5")
  | select(.publisher_sha256 | test("^[0-9a-f]{64}$"))
  | select(.sandbox_sha256 | test("^[0-9a-f]{64}$"))
  | select(.splitctl_sha256 | test("^[0-9a-f]{64}$"))
  | select(.jankurai_sha256 | test("^[0-9a-f]{64}$"))
  | select((.security_tool_sha256 | keys) == ["actionlint", "grype", "syft"])
  | select(all(.security_tool_sha256[]; test("^[0-9a-f]{64}$")))
  | select(.grype_db_root | type == "string" and startswith("/"))
  | select(.grype_db_inventory_sha256 | test("^[0-9a-f]{64}$"))
  | select(.forge_git_base | type == "string" and length > 0)
  | select(.control_remote | type == "string" and length > 0)
  | select(.request_root | type == "string" and startswith("/"))
  | select(.native_evidence_root | type == "string" and startswith("/"))
  | select(.proof_evidence_root | type == "string" and startswith("/"))
  | select(.max_seal_age_seconds | type == "number" and . >= 1 and . <= 300)
  | select(.token_file | type == "string" and startswith("/"))
  | select((.control_ref // "refs/heads/main") | type == "string")
  | select((.bootstrap_commit // "") | type == "string")
  | select((.bootstrap_expires_at // "") | type == "string")
  | select(has("token") | not)' "$config" >/dev/null \
  || fail 'invalid publisher config schema'

control_ref="$(jq -er '.control_ref // "refs/heads/main"' "$config")"
bootstrap_commit="$(jq -er '.bootstrap_commit // ""' "$config")"
bootstrap_expires_at="$(jq -er '.bootstrap_expires_at // ""' "$config")"
validate_control_authority \
  "$control_ref" "$bootstrap_commit" "$bootstrap_expires_at"

token_file="$(jq -er '.token_file' "$config")"
[[ ! -L "$token_file" \
  && "$(realpath -e -- "$token_file" 2>/dev/null)" == "$token_file" \
  && "$(stat -c '%u:%a:%h' -- "$token_file" 2>/dev/null)" == '0:600:1' ]] \
  || fail 'publisher token file must be canonical, root-owned mode 0600, and single-link'

publisher_sha="$(sha256sum -- "$publisher_path" | cut -d' ' -f1)"
expected_publisher_sha="$(jq -er '.publisher_sha256' "$config")"
[[ "$publisher_sha" == "$expected_publisher_sha" ]] \
  || fail 'publisher digest/config mismatch'
splitctl_sha="$(sha256sum -- "$splitctl_path" | cut -d' ' -f1)"
[[ "$splitctl_sha" == "$(jq -er '.splitctl_sha256' "$config")" ]] \
  || fail 'splitctl digest/config mismatch'
jankurai_sha="$(sha256sum -- "$jankurai_path" | cut -d' ' -f1)"
[[ "$jankurai_sha" == "$(jq -er '.jankurai_sha256' "$config")" \
  && "$("$jankurai_path" --version)" == 'jankurai 1.6.11' ]] \
  || fail 'governed Jankurai digest/version mismatch'
security_tool_sha256="$(jq -c '.security_tool_sha256' "$config")"
for tool in "${security_tool_names[@]}"; do
  tool_path="$install_dir/security-$tool"
  [[ ! -L "$tool_path" \
    && "$(stat -c '%u:%g:%a:%h' -- "$tool_path" 2>/dev/null)" == '0:0:555:1' \
    && "$(sha256sum -- "$tool_path" | cut -d' ' -f1)" \
      == "$(jq -er --arg tool "$tool" '.security_tool_sha256[$tool]' "$config")" ]] \
    || fail "installed security tool digest/metadata mismatch: $tool"
done
request_root="$(realpath -e -- "$(jq -er '.request_root' "$config")")" \
  || fail 'request root is unavailable'
[[ ! -L "$request_root" \
  && "$(stat -c '%u:%a' -- "$request_root")" == '0:700' ]] \
  || fail 'request root must be root-owned mode 0700'
native_evidence_root="$(realpath -e -- \
  "$(jq -er '.native_evidence_root' "$config")")" \
  || fail 'durable native evidence root is unavailable'
case "$native_evidence_root" in
  /tmp | /tmp/*) fail 'durable native evidence root cannot use /tmp' ;;
esac
[[ ! -L "$native_evidence_root" \
  && "$(stat -c '%u:%g:%a' -- "$native_evidence_root")" == '0:0:700' ]] \
  || fail 'durable native evidence root must be root-owned mode 0700'
proof_evidence_root="$(realpath -e -- \
  "$(jq -er '.proof_evidence_root' "$config")")" \
  || fail 'durable proof evidence root is unavailable'
case "$proof_evidence_root" in
  /tmp | /tmp/*) fail 'durable proof evidence root cannot use /tmp' ;;
esac
[[ "$proof_evidence_root" != "$native_evidence_root" \
  && ! -L "$proof_evidence_root" \
  && "$(stat -c '%u:%g:%a' -- "$proof_evidence_root")" == '0:0:700' ]] \
  || fail 'durable proof evidence root must be distinct root-owned mode 0700'
case "$proof_evidence_root/" in
  "$native_evidence_root/"*) fail 'durable evidence roots cannot be nested' ;;
esac
case "$native_evidence_root/" in
  "$proof_evidence_root/"*) fail 'durable evidence roots cannot be nested' ;;
esac

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
  'select(.schema_version == "jain.host-ci-root-state/v5")
   | select(.request_id == $request_id and .status == "sealed")
   | select(.nonce | test("^[0-9a-f]{64}$"))
   | select(.result_sha256 | test("^[0-9a-f]{64}$"))
   | select(.root_seal | test("^[0-9a-f]{64}$"))
   | select(.sealed_at | type == "number")
   | select(.control_repository | test("^[a-z0-9][a-z0-9._-]*/[a-z0-9][a-z0-9._-]*$"))
   | select(.control_remote | type == "string" and length > 0)
   | select(.control_api_identity.host == "jeryu")
   | select(.control_api_identity.owner | type == "string")
   | select(.control_api_identity.name | type == "string")
   | select(.control_api_identity.default_branch == "main")
   | select(.control_api_identity.clone_http_url | type == "string")
   | select(.control_ref | type == "string")
   | select(.control_plane_commit | test("^[0-9a-f]{40}$"))
   | select(.bootstrap_expires_at | type == "string")
   | select(.publisher_sha256 | test("^[0-9a-f]{64}$"))
   | select(.sandbox_sha256 | test("^[0-9a-f]{64}$"))
   | select(.splitctl_sha256 | test("^[0-9a-f]{64}$"))
   | select(.jankurai_sha256 | test("^[0-9a-f]{64}$"))
   | select((.security_tool_sha256 | keys) == ["actionlint", "grype", "syft"])
   | select(all(.security_tool_sha256[]; test("^[0-9a-f]{64}$")))
   | select(.grype_db_root | type == "string" and startswith("/"))
   | select(.grype_db_inventory_sha256 | test("^[0-9a-f]{64}$"))
   | select(.native_evidence_root | type == "string")
   | select(.proof_evidence_root | type == "string")' "$state" >/dev/null \
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
proof_receipt_sha="$(jq -er '.proof_receipt_sha256 | select(test("^[0-9a-f]{64}$"))' \
  "$result")" || fail 'root result lacks a proof receipt digest'
expected_seal="$({
  printf '%s\n%s\n%s\n%s\n%s\n' \
    "$nonce" "$result_sha" "$proof_receipt_sha" "$sealed_at" "$request_id"
} | sha256sum | cut -d' ' -f1)"
[[ "$expected_seal" == "$(jq -er '.root_seal' "$state")" ]] \
  || fail 'root result seal mismatch'
[[ "$(jq -er '.publisher_sha256' "$state")" == "$publisher_sha" \
  && "$(jq -er '.sandbox_sha256' "$state")" \
    == "$(jq -er '.sandbox_sha256' "$config")" \
  && "$(jq -er '.splitctl_sha256' "$state")" == "$splitctl_sha" \
  && "$(jq -er '.jankurai_sha256' "$state")" == "$jankurai_sha" \
  && "$(jq -c '.security_tool_sha256' "$state")" == "$security_tool_sha256" \
  && "$(jq -er '.grype_db_root' "$state")" \
    == "$(jq -er '.grype_db_root' "$config")" \
  && "$(jq -er '.grype_db_inventory_sha256' "$state")" \
    == "$(jq -er '.grype_db_inventory_sha256' "$config")" \
  && "$(jq -er '.native_evidence_root' "$state")" \
    == "$native_evidence_root" \
  && "$(jq -er '.proof_evidence_root' "$state")" \
    == "$proof_evidence_root" ]] \
  || fail 'root request broker binding mismatch'

jq -e --arg request_id "$request_id" \
  'select(.schema_version == "jain.host-ci-root-result/v5")
   | select(.request_id == $request_id)
   | select(.control_repository | test("^[a-z0-9][a-z0-9._-]*/[a-z0-9][a-z0-9._-]*$"))
   | select(.control_remote | type == "string" and length > 0)
   | select(.control_api_identity.host == "jeryu")
   | select(.control_api_identity.owner | type == "string")
   | select(.control_api_identity.name | type == "string")
   | select(.control_api_identity.default_branch == "main")
   | select(.control_api_identity.clone_http_url | type == "string")
   | select(.control_ref | type == "string")
   | select(.control_plane_commit | test("^[0-9a-f]{40}$"))
   | select(.owner | test("^[a-z0-9][a-z0-9-]*$"))
   | select(.repository | test("^[a-z0-9][a-z0-9-]*$"))
   | select(.head_sha | test("^[0-9a-f]{40}$"))
   | select(.required_check | test("^[a-z0-9][a-z0-9-]*/required$"))
   | select(.conclusion == "success" or .conclusion == "failure")
   | select(.runner_exit_code | type == "number")
   | select(.native_evidence_required | type == "boolean")
   | select(.native_evidence_dir | type == "string")
   | select(.native_evidence_sha256 | type == "string")
   | select(.proof_evidence_required == true)
   | select(.proof_evidence_dir | type == "string" and startswith("/"))
   | select(.proof_receipt_path | type == "string" and startswith("/"))
   | select(.proof_receipt_sha256 | test("^[0-9a-f]{64}$"))
   | select(.proof_report_path | type == "string" and startswith("/"))
   | select(.proof_report_sha256 | test("^[0-9a-f]{64}$"))
   | select(.proof_status == "pass" or .proof_status == "fail")
   | select(.proof_attempt_id | test("^[A-Za-z0-9_.-]+$"))
   | select(.proof_auditor_exit_code | type == "number")
   | select(.proof_validator_exit_code | type == "number")' "$result" >/dev/null \
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
  ops/ci/host-ci-sandbox.sh ops/ci/native-runtime.sh \
  ops/ci/host-ci-evidence.sh ops/ci/host-ci-proof-evidence.sh \
  tools/splitctl/src/main.rs tools/splitctl/src/jeryu_client.rs; do
  [[ -f "$control_root/$critical" && ! -L "$control_root/$critical" \
    && "$(stat -c '%u' -- "$control_root/$critical")" == 0 ]] \
    || fail "unsafe immutable control input: $critical"
done
reexec_state="$request_dir/worker-authority/reexec-state.json"
[[ -f "$reexec_state" && ! -L "$reexec_state" \
  && "$(stat -c '%u:%a:%h' -- "$reexec_state")" == '0:444:1' ]] \
  || fail 'unsafe worker re-exec authority'
jq -e '
  select(.schema_version == "jain.host-ci-reexec/v5")
  | select(.source_root == "/opt/jain-ci/authority/control-plane")
  | select(.exact_root == "/opt/jain-ci/authority/control-plane")
  | select(.control_repository | type == "string")
  | select(.control_remote | type == "string")
  | select(.control_api_identity | type == "object")
  | select(.control_ref | type == "string")
  | select(.control_plane_commit | test("^[0-9a-f]{40}$"))' \
  "$reexec_state" >/dev/null || fail 'invalid worker re-exec authority'
control_commit="$(jq -er '.control_plane_commit' "$result")"
[[ "$control_commit" == "$(jq -er '.control_plane_commit' "$state")" ]] \
  || fail 'control commit differs across root artifacts'
control_repository="$(jq -er '.control_repository' "$state")"
control_remote="$(jq -er '.control_remote' "$config")"
control_api_identity="$(jq -c '.control_api_identity' "$state")"
[[ "$(jq -er '.control_ref' "$state")" == "$control_ref" ]] \
  || fail 'control ref differs across root artifacts'
[[ "$(jq -er '.bootstrap_expires_at' "$state")" == "$bootstrap_expires_at" ]] \
  || fail 'bootstrap expiry differs across root artifacts'
if [[ "$control_ref" != refs/heads/main && "$bootstrap_commit" != "$control_commit" ]]; then
  fail 'bootstrap control commit differs from the sealed result'
fi
for artifact in "$result" "$reexec_state"; do
  [[ "$(jq -er '.control_repository' "$artifact")" == "$control_repository" \
    && "$(jq -er '.control_remote' "$artifact")" == "$control_remote" \
    && "$(jq -c '.control_api_identity' "$artifact")" == "$control_api_identity" \
    && "$(jq -er '.control_ref' "$artifact")" == "$control_ref" \
    && "$(jq -er '.control_plane_commit' "$artifact")" == "$control_commit" ]] \
    || fail 'control authority differs across broker-owned evidence'
done
[[ "$control_repository" \
    == "$(jq -er '.owner + "/" + .name' <<<"$control_api_identity")" \
  && "$(jq -er '.clone_http_url' <<<"$control_api_identity")" \
    == "/git/$control_repository.git" \
  && "$control_remote" == */git/"$control_repository".git ]] \
  || fail 'sealed control API identity is not canonical authority'
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
repo_authority_json="$("$splitctl_path" host-ci-authority \
  --manifest "$manifest" --repo "$repo")" \
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
owner="$(jq -er '.owner' "$result")"
required_check="$(jq -er '.required_check' "$result")"
[[ "$owner" == "$protected_owner" && "$required_check" == "$protected_check" ]] \
  || fail 'root result owner/check differs from manifest authority'

# Derive native evidence policy from the root-owned reviewed policy code. A
# worker-supplied boolean is never an authorization input.
# shellcheck source=ops/ci/native-runtime.sh
source "$control_root/ops/ci/native-runtime.sh"
# shellcheck source=ops/ci/host-ci-evidence.sh
source "$control_root/ops/ci/host-ci-evidence.sh"
# shellcheck source=ops/ci/host-ci-proof-evidence.sh
source "$control_root/ops/ci/host-ci-proof-evidence.sh"
derived_required=false
if jain_native_check_requires_evidence "$repo" "$required_check" "$protected_check"; then
  derived_required=true
fi
[[ "$(jq -er '.native_evidence_required' "$result")" == "$derived_required" ]] \
  || fail 'root result attempted a native evidence policy downgrade'
conclusion="$(jq -er '.conclusion' "$result")"
head_sha="$(jq -er '.head_sha' "$result")"
proof_evidence_dir="$(jq -er '.proof_evidence_dir' "$result")"
proof_evidence_resolved="$(realpath -e -- "$proof_evidence_dir" 2>/dev/null || true)"
proof_check_slug="${required_check//[^A-Za-z0-9_.-]/_}"
expected_proof_dir="$proof_evidence_root/$owner/$repo/$proof_check_slug/$request_id"
[[ "$proof_evidence_resolved" == "$expected_proof_dir" \
  && "$(jq -er '.proof_receipt_path' "$result")" \
    == "$proof_evidence_resolved/receipt.json" \
  && "$(jq -er '.proof_report_path' "$result")" \
    == "$proof_evidence_resolved/report.json" ]] \
  || fail 'root result proof evidence escaped configured durable authority'
proof_status="$(jq -er '.proof_status' "$result")"
proof_attempt_id="$(jq -er '.proof_attempt_id' "$result")"
proof_auditor_rc="$(jq -er '.proof_auditor_exit_code' "$result")"
proof_validator_rc="$(jq -er '.proof_validator_exit_code' "$result")"
jain_host_ci_verify_promoted_proof_evidence "$proof_evidence_resolved" \
  "$owner" "$repo" "$head_sha" "$required_check" "$proof_attempt_id" \
  "$jankurai_sha" "$proof_status" \
  || fail 'root result proof evidence is not immutable root authority'
[[ "$(sha256sum -- "$proof_evidence_resolved/receipt.json" | cut -d' ' -f1)" \
    == "$proof_receipt_sha" \
  && "$(sha256sum -- "$proof_evidence_resolved/report.json" | cut -d' ' -f1)" \
    == "$(jq -er '.proof_report_sha256' "$result")" ]] \
  || fail 'root result proof evidence digest mismatch'
if [[ "$conclusion" == success ]]; then
  [[ "$(jq -er '.runner_exit_code' "$result")" == 0 ]] \
    || fail 'success result has a nonzero worker exit'
  [[ "$proof_status" == pass && "$proof_auditor_rc" == 0 \
    && "$proof_validator_rc" == 0 ]] \
    || fail 'success result lacks a passing exact-SHA Jankurai proof'
  evidence_dir="$(jq -er '.native_evidence_dir' "$result")"
  evidence_sha="$(jq -er '.native_evidence_sha256' "$result")"
  required_int=0
  [[ "$derived_required" == true ]] && required_int=1
  if [[ "$derived_required" == true ]]; then
    evidence_resolved="$(realpath -e -- "$evidence_dir" 2>/dev/null || true)"
    case "$evidence_resolved" in
      "$native_evidence_root"/*) ;;
      *) fail 'root result native evidence escaped configured durable root' ;;
    esac
    jain_host_ci_verify_promoted_evidence "$evidence_resolved" \
      || fail 'root result native evidence is not immutable root authority'
  fi
  jain_verify_native_check_evidence success "$required_int" \
    "$evidence_dir" "$evidence_sha" "$head_sha" "$required_check" \
    "$control_root" "$control_commit" \
    || fail 'root result native evidence validation failed'
else
  [[ "$proof_status" == fail && "$proof_validator_rc" != 0 ]] \
    || fail 'failure result lacks a failed exact-SHA Jankurai receipt'
fi

[[ "$(jq -er '.control_remote' "$state")" == "$control_remote" ]] \
  || fail 'root request control remote binding mismatch'
forge_git_base="$(jq -er '.forge_git_base' "$config")"
product_remote="${forge_git_base%/}/$owner/$repo.git"
"${safe_git[@]}" ls-remote --exit-code "$product_remote" 2>/dev/null \
  | awk -v head="$head_sha" '$1 == head { found=1 } END { exit !found }' \
  || fail 'head is not an advertised authoritative product ref'

write_status() {
  local status="${1:?status required}" tmp="$request_dir/root-state.tmp.$$"
  jq --arg status "$status" '.status=$status' "$state" >"$tmp"
  chmod 0600 "$tmp"
  chown root:root "$tmp"
  mv -f -- "$tmp" "$state"
}
write_status publishing

# This is the final pre-publication admission gate. It repeats both the API
# identity and exact-ref readback after the one-shot state transition and
# immediately before splitctl can issue the first forge POST.
validate_control_authority \
  "$control_ref" "$bootstrap_commit" "$bootstrap_expires_at"
current_control_authority="$("$splitctl_path" jeryu-local authority-readback \
  --repo "$control_repository" --remote "$control_remote" \
  --ref "$control_ref" --expected-head "$control_commit" \
  --token-file "$token_file")" \
  || fail 'cannot repeat authenticated control authority before publication'
jq -e --arg repository "$control_repository" --arg remote "$control_remote" \
  --arg ref "$control_ref" --arg commit "$control_commit" \
  --argjson api_identity "$control_api_identity" '
  select(.schema_version == "jain.jeryu-authority-readback/v1")
  | select(.repository == $repository and .remote == $remote)
  | select(.api_identity == $api_identity)
  | select(.ref == $ref and .commit == $commit)' \
  <<<"$current_control_authority" >/dev/null \
  || fail 'pre-publication control authority differs from sealed evidence'

proof_run_id="$(jq -er '.run_id' "$proof_evidence_resolved/receipt.json")"
proof_score="$(jq -er '.score' "$proof_evidence_resolved/receipt.json")"
proof_hard="$(jq -er '.hard_findings' "$proof_evidence_resolved/receipt.json")"
proof_caps="$(jq -er '.caps_applied' "$proof_evidence_resolved/receipt.json")"
proof_summary="receipt_sha256=$proof_receipt_sha attempt_id=$proof_attempt_id run_id=$proof_run_id auditor_sha256=$jankurai_sha proof_status=$proof_status score=$proof_score hard_findings=$proof_hard caps_applied=$proof_caps root_seal=$(jq -er '.root_seal' "$state")"
description="$required_check root-seal=$(jq -er '.root_seal' "$state" | cut -c1-16)"
publish_rc=0
"$splitctl_path" jeryu-publish-host-ci \
  --token-file "$token_file" --repo "$owner/$repo" --head-sha "$head_sha" \
  --required-check "$required_check" --conclusion "$conclusion" \
  --proof-summary "$proof_summary" \
  --proof-receipt-sha256 "$proof_receipt_sha" \
  --proof-attempt-id "$proof_attempt_id" \
  --status-description "$description" --apply || publish_rc=$?
case "$publish_rc" in
  0) ;;
  41) write_status failed; fail 'Jankurai proof publication failed before proof POST' ;;
  42) write_status consumed; fail 'publication failed after proof POST; request consumed' ;;
  *) write_status consumed; fail "publisher returned unexpected status $publish_rc; request consumed" ;;
esac
write_status consumed
printf '[host-ci-publisher] consumed one-shot %s for %s/%s @ %.12s\n' \
  "$request_id" "$owner" "$repo" "$head_sha" >&2
