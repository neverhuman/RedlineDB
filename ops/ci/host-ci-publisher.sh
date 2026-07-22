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
  (( expires >= now && expires - now <= 7200 )) \
    || fail 'bootstrap control authority is expired or exceeds two hours'
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
   | select(.control_plane_commit | test("^[0-9a-f]{40}$"))
   | select(.control_ref | type == "string")
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
   | select(.proof_evidence_root | type == "string")
   | select(.product_base_commit | test("^[0-9a-f]{40}$"))
   | select(.product_release_tag_ref | type == "string")
   | select(.product_release_tag_commit | type == "string")
   | select(.sibling_sources_required | type == "boolean")
   | select(.sibling_sources_sha256 | type == "string")
   | select(.cuda_compute_capability_required | type == "boolean")
   | select(.cuda_capability_record_path | type == "string")
   | select(.cuda_capability_record_sha256 | type == "string")
   | select(.cuda_compute_capability | type == "string")' "$state" >/dev/null \
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
   | select(.product_base_commit | test("^[0-9a-f]{40}$"))
   | select(.product_release_tag_ref | type == "string")
   | select(.product_release_tag_commit | type == "string")
   | select(.sibling_sources_required | type == "boolean")
   | select(.sibling_sources_sha256 | type == "string")
   | select(.cuda_compute_capability_required | type == "boolean")
   | select(.cuda_capability_record_path | type == "string")
   | select(.cuda_capability_record_sha256 | type == "string")
   | select(.cuda_compute_capability | type == "string")
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
  ops/ci/pnpm-runtime.sh ops/ci/pnpm-store.lock.json \
  ops/ci/host-ci-evidence.sh ops/ci/host-ci-proof-evidence.sh \
  contracts/ci-plan.schema.json contracts/ci-lane-result.schema.json \
  contracts/ci-performance.schema.json \
  contracts/host-ci-result-v6.schema.json \
  contracts/host-ci-evidence-v6.schema.json \
  tools/splitctl/src/main.rs tools/splitctl/src/ci.rs \
  tools/splitctl/src/jeryu_client.rs; do
  [[ -f "$control_root/$critical" && ! -L "$control_root/$critical" \
    && "$(stat -c '%u' -- "$control_root/$critical")" == 0 ]] \
    || fail "unsafe immutable control input: $critical"
done
control_commit="$(jq -er '.control_plane_commit' "$result")"
[[ "$control_commit" == "$(jq -er '.control_plane_commit' "$state")" ]] \
  || fail 'control commit differs across root artifacts'
[[ "$(jq -er '.control_ref' "$state")" == "$control_ref" ]] \
  || fail 'control ref differs across root artifacts'
[[ "$(jq -er '.bootstrap_expires_at' "$state")" == "$bootstrap_expires_at" ]] \
  || fail 'bootstrap expiry differs across root artifacts'
if [[ "$control_ref" != refs/heads/main && "$bootstrap_commit" != "$control_commit" ]]; then
  fail 'bootstrap control commit differs from the sealed result'
fi
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
  | select(.release_cuda_compute_capability_required | type == "boolean")
  | [.forge_owner, .required_check,
      (.release_cuda_compute_capability_required | tostring)] | @tsv
' <<<"$repo_authority_json")" || fail 'invalid repository authority result'
IFS=$'\t' read -r protected_owner protected_check cuda_required \
  <<<"$repo_authority"
owner="$(jq -er '.owner' "$result")"
required_check="$(jq -er '.required_check' "$result")"
[[ "$owner" == "$protected_owner" && "$required_check" == "$protected_check" ]] \
  || fail 'root result owner/check differs from manifest authority'

product_release_tag_ref="$(jq -er '.product_release_tag_ref' "$result")"
product_release_tag_commit="$(jq -er '.product_release_tag_commit' "$result")"
product_base_commit="$(jq -er '.product_base_commit' "$result")"
[[ "$(jq -er '.product_base_commit' "$state")" == "$product_base_commit" \
  && "$(jq -er '.product_release_tag_ref' "$state")" \
    == "$product_release_tag_ref" \
  && "$(jq -er '.product_release_tag_commit' "$state")" \
    == "$product_release_tag_commit" ]] \
  || fail 'product release-tag binding differs across sealed authority'
product_authority="$request_dir/product-authority"
[[ -d "$product_authority/.git" && ! -L "$product_authority" \
  && ! -L "$product_authority/.git" ]] \
  || fail 'sealed product authority is missing'
retained_product_tags="$("${safe_git[@]}" -C "$product_authority" \
  for-each-ref --format='%(refname)' refs/tags)" \
  || fail 'cannot enumerate sealed product release tags'
head_sha="$(jq -er '.head_sha' "$result")"
if [[ -n "$product_release_tag_ref" ]]; then
  [[ "$product_release_tag_ref" \
      =~ ^refs/tags/${repo}-v[0-9A-Za-z.-]+-split\.[0-9]+$ \
    && "$product_release_tag_commit" =~ ^[0-9a-f]{40}$ \
    && "$retained_product_tags" == "$product_release_tag_ref" \
    && "$("${safe_git[@]}" -C "$product_authority" rev-parse --verify \
      "$product_release_tag_ref")" == "$product_release_tag_commit" \
    && "$("${safe_git[@]}" -C "$product_authority" rev-parse --verify \
      "$product_release_tag_ref^{commit}")" == "$product_release_tag_commit" ]] \
    && "${safe_git[@]}" -C "$product_authority" merge-base --is-ancestor \
      "$product_release_tag_commit" "$head_sha" \
    || fail 'sealed product release tag is mismatched or not an ancestor'
else
  [[ -z "$product_release_tag_commit" && -z "$retained_product_tags" ]] \
    || fail 'tag-free product authority retained unexpected tags'
fi
product_base_authority="$request_dir/product-main-authority"
[[ -d "$product_base_authority/.git" && ! -L "$product_base_authority" \
  && ! -L "$product_base_authority/.git" \
  && ! -e "$product_base_authority/.git/commondir" \
  && ! -e "$product_base_authority/.git/worktrees" \
  && ! -e "$product_base_authority/.git/objects/info/alternates" \
  && "$("${safe_git[@]}" -C "$product_base_authority" \
    rev-parse --absolute-git-dir)" == "$product_base_authority/.git" \
  && "$("${safe_git[@]}" -C "$product_base_authority" \
    rev-parse --verify 'HEAD^{commit}')" == "$product_base_commit" \
  && -z "$("${safe_git[@]}" -C "$product_base_authority" remote)" \
  && -z "$("${safe_git[@]}" -C "$product_base_authority" \
    for-each-ref --format='%(refname)' refs/tags)" \
  && -z "$("${safe_git[@]}" -C "$product_base_authority" \
    status --porcelain=v1 --untracked-files=all)" \
  && "$("${safe_git[@]}" -C "$product_authority" \
    rev-parse --verify "$product_base_commit^{commit}")" \
    == "$product_base_commit" ]] \
  && "${safe_git[@]}" -C "$product_authority" merge-base --is-ancestor \
    "$product_base_commit" "$head_sha" \
  || fail 'sealed product protected-main authority changed or is not an ancestor'

family_root="$(realpath -e -- "$(jq -er '.split_root' \
  "$request_dir/caller-request.json")")" \
  || fail 'cannot resolve sealed family root'
sibling_request="$(jq -r '.environment.JAIN_NEEDS_SIBLINGS // "0"' \
  "$request_dir/caller-request.json")" \
  || fail 'cannot read sealed sibling request policy'
[[ "$sibling_request" == 0 || "$sibling_request" == 1 ]] \
  || fail 'sealed sibling request policy is invalid'
expected_sibling_sources=false
if [[ "$repo" == jain-deploy || "$sibling_request" == 1 ]]; then
  expected_sibling_sources=true
fi
sibling_sources_required="$(jq -r '.sibling_sources_required' "$result")"
sibling_sources_sha="$(jq -er '.sibling_sources_sha256' "$result")"
[[ "$sibling_sources_required" == "$expected_sibling_sources" \
  && "$(jq -r '.sibling_sources_required' "$state")" \
    == "$expected_sibling_sources" \
  && "$(jq -er '.sibling_sources_sha256' "$state")" \
    == "$sibling_sources_sha" ]] \
  || fail 'sibling source binding differs across sealed authority'
sibling_sources_path="$request_dir/worker-authority/sibling-sources.json"
if [[ "$sibling_sources_required" == true ]]; then
  [[ "$sibling_sources_sha" =~ ^[0-9a-f]{64}$ \
    && -f "$sibling_sources_path" && ! -L "$sibling_sources_path" \
    && "$(stat -c '%u:%g:%a:%h' -- "$sibling_sources_path")" \
      == '0:0:444:1' \
    && "$(sha256sum -- "$sibling_sources_path" | cut -d' ' -f1)" \
      == "$sibling_sources_sha" ]] \
    || fail 'sealed sibling source inventory is missing or changed'
  jq -e --arg request_id "$request_id" --arg control "$control_commit" \
    --arg owner "$owner" --arg repository "$repo" \
    --arg head "$(jq -er '.head_sha' "$result")" \
    --arg check "$required_check" --arg family_root "$family_root" '
    select(.schema_version == "jain.host-ci-sibling-sources/v1")
    | select(.request_id == $request_id and .control_plane_commit == $control)
    | select(.owner == $owner and .repository == $repository)
    | select(.head_sha == $head and .required_check == $check)
    | select(.reference == "refs/heads/main")
    | select((.sources | type) == "array" and (.sources | length) > 0)
    | select(.sources == (.sources | sort_by(.repository)))
    | select((.sources | map(.repository) | unique | length)
        == (.sources | length))
    | select(all(.sources[];
        (.repository | test("^[a-z0-9][a-z0-9-]*$"))
        and .owner == "veox"
        and .remote == ("http://127.0.0.1:8787/git/veox/" + .repository + ".git")
        and .reference == "refs/heads/main"
        and (.commit | test("^[0-9a-f]{40}$"))
        and (.tree | test("^[0-9a-f]{40}$"))
        and (.inventory_sha256 | test("^[0-9a-f]{64}$"))
        and (.entry_count | type) == "number" and .entry_count >= 0
        and .mount_path == ($family_root + "/" + .repository)))' \
    "$sibling_sources_path" >/dev/null \
    || fail 'sealed sibling source inventory has invalid identities'
else
  [[ -z "$sibling_sources_sha" && ! -e "$sibling_sources_path" ]] \
    || fail 'sibling-free request carried unexpected source authority'
fi

# Derive native evidence policy from the root-owned reviewed policy code. A
# worker-supplied boolean is never an authorization input.
# shellcheck source=ops/ci/native-runtime.sh
source "$control_root/ops/ci/native-runtime.sh"
# shellcheck source=ops/ci/host-ci-evidence.sh
source "$control_root/ops/ci/host-ci-evidence.sh"
# shellcheck source=ops/ci/host-ci-proof-evidence.sh
source "$control_root/ops/ci/host-ci-proof-evidence.sh"
release_cargo_policy="$("$splitctl_path" release-cargo-commands \
  --manifest "$manifest" --repo "$repo")" \
  || fail 'release Cargo policy is absent or invalid'
[[ "$(jq -r '.release_cuda_compute_capability_required' \
    <<<"$release_cargo_policy")" == "$cuda_required" ]] \
  || fail 'release CUDA capability policy disagrees across authorities'
for field in cuda_compute_capability_required cuda_capability_record_path \
  cuda_capability_record_sha256 cuda_compute_capability; do
  [[ "$(jq -r ".$field" "$state")" == "$(jq -r ".$field" "$result")" ]] \
    || fail "CUDA capability binding differs across root artifacts: $field"
done
cuda_record="$(jq -er '.cuda_capability_record_path' "$result")"
cuda_record_sha="$(jq -er '.cuda_capability_record_sha256' "$result")"
cuda_cap="$(jq -er '.cuda_compute_capability' "$result")"
if [[ "$cuda_required" == true ]]; then
  [[ "$(jq -r '.cuda_compute_capability_required' "$result")" == true \
    && "$cuda_record" == "$request_dir/worker-authority/cuda-capability.json" \
    && "$cuda_record_sha" =~ ^[0-9a-f]{64}$ \
    && "$cuda_cap" =~ ^[1-9][0-9]{1,2}$ ]] \
    || fail 'required CUDA capability binding is missing or malformed'
  detector_sha="$(jq -er '.detector.sha256' "$cuda_record")" \
    || fail 'CUDA capability record lacks detector identity'
  jain_verify_cuda_capability_record "$cuda_record" "$cuda_record_sha" \
    "$request_id" "$(jq -er '.control_plane_commit' "$result")" \
    "$(jq -er '.owner' "$result")" "$repo" \
    "$(jq -er '.head_sha' "$result")" \
    "$(jq -er '.required_check' "$result")" \
    "$detector_sha" "$cuda_cap" 0 \
    || fail 'root CUDA capability record validation failed'
else
  [[ "$cuda_required" == false \
    && "$(jq -r '.cuda_compute_capability_required' "$result")" == false \
    && -z "$cuda_record" && -z "$cuda_record_sha" && -z "$cuda_cap" ]] \
    || fail 'CPU-only policy carried CUDA capability authority'
fi
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
  if [[ "$derived_required" == true ]]; then
    [[ "$(jq -r '.cuda_compute_capability_required' \
        "$evidence_resolved/receipt.json")" == "$cuda_required" \
      && "$(jq -er '.cuda_compute_capability' \
        "$evidence_resolved/receipt.json")" == "$cuda_cap" \
      && "$(jq -er '.cuda_capability_record_sha256' \
        "$evidence_resolved/receipt.json")" == "$cuda_record_sha" ]] \
      || fail 'native evidence disagrees with root CUDA capability authority'
  fi
else
  [[ "$proof_status" == fail && "$proof_validator_rc" != 0 ]] \
    || fail 'failure result lacks a failed exact-SHA Jankurai receipt'
fi

control_remote="$(jq -er '.control_remote' "$config")"
[[ "$(jq -er '.control_remote' "$state")" == "$control_remote" ]] \
  || fail 'root request control remote binding mismatch'
if [[ "$conclusion" == success ]]; then
  "$splitctl_path" jeryu-local ref-readback \
    --repo veox/jain-split-ops --remote "$control_remote" \
    --ref "$control_ref" --expected-head "$control_commit" \
    --token-file "$token_file" >/dev/null \
    || fail 'success authority no longer equals the sealed control commit'
  if [[ "$sibling_sources_required" == true ]]; then
    while IFS=$'\t' read -r sibling sibling_owner sibling_remote \
      sibling_commit sibling_tree sibling_inventory sibling_count; do
      sibling_stage="$request_dir/worker-authority/sibling-checkouts/$sibling"
      sibling_authority_json="$("$splitctl_path" host-ci-authority \
        --manifest "$manifest" --repo "$sibling")" \
        || fail "cannot re-read sibling manifest authority: $sibling"
      [[ "$(jq -er '[.forge_owner,.remote] | @tsv' \
          <<<"$sibling_authority_json")" \
          == "$sibling_owner"$'\t'"$sibling_remote" \
        && -d "$sibling_stage/.git" && ! -L "$sibling_stage" \
        && ! -L "$sibling_stage/.git" \
        && "$("${safe_git[@]}" -C "$sibling_stage" \
          rev-parse --verify 'HEAD^{commit}')" == "$sibling_commit" \
        && "$("${safe_git[@]}" -C "$sibling_stage" \
          rev-parse --verify 'HEAD^{tree}')" == "$sibling_tree" \
        && "$({ LC_ALL=C "${safe_git[@]}" -C "$sibling_stage" \
          ls-tree -r --full-tree "$sibling_commit"; } \
          | sha256sum | cut -d' ' -f1)" == "$sibling_inventory" \
        && "$("${safe_git[@]}" -C "$sibling_stage" \
          ls-tree -r --full-tree "$sibling_commit" | wc -l \
            | awk '{print $1}')" \
          == "$sibling_count" \
        && -z "$("${safe_git[@]}" -C "$sibling_stage" remote)" \
        && -z "$("${safe_git[@]}" -C "$sibling_stage" \
          status --porcelain=v1 --untracked-files=all)" ]] \
        || fail "sealed sibling source changed before publication: $sibling"
      "$splitctl_path" jeryu-local ref-readback \
        --repo "$sibling_owner/$sibling" --remote "$sibling_remote" \
        --ref refs/heads/main --expected-head "$sibling_commit" \
        --token-file "$token_file" >/dev/null \
        || fail "sibling protected main moved before publication: $sibling"
    done < <(jq -r '.sources[]
      | [.repository,.owner,.remote,.commit,.tree,.inventory_sha256,
          (.entry_count | tostring)] | @tsv' "$sibling_sources_path")
  fi
fi
forge_git_base="$(jq -er '.forge_git_base' "$config")"
product_remote="${forge_git_base%/}/$owner/$repo.git"
"$splitctl_path" jeryu-local ref-readback \
  --repo "$owner/$repo" --remote "$product_remote" \
  --ref refs/heads/main --expected-head "$product_base_commit" \
  --token-file "$token_file" >/dev/null \
  || fail 'product protected main moved before publication'
"$splitctl_path" jeryu-local ref-readback \
  --repo "$owner/$repo" --remote "$product_remote" \
  --expected-head "$head_sha" --token-file "$token_file" >/dev/null \
  || fail 'head is not an advertised authoritative product ref'
if [[ "$conclusion" == success && -n "$product_release_tag_ref" ]]; then
  "$splitctl_path" jeryu-local ref-readback \
    --repo "$owner/$repo" --remote "$product_remote" \
    --ref "$product_release_tag_ref" \
    --expected-head "$product_release_tag_commit" \
    --token-file "$token_file" >/dev/null \
    || fail 'product release tag moved before success publication'
fi

write_status() {
  local status="${1:?status required}" tmp="$request_dir/root-state.tmp.$$"
  jq --arg status "$status" '.status=$status' "$state" >"$tmp"
  chmod 0600 "$tmp"
  chown root:root "$tmp"
  mv -f -- "$tmp" "$state"
}
write_status publishing

proof_run_id="$(jq -er '.run_id' "$proof_evidence_resolved/receipt.json")"
proof_score="$(jq -er '.score' "$proof_evidence_resolved/receipt.json")"
proof_hard="$(jq -er '.hard_findings' "$proof_evidence_resolved/receipt.json")"
proof_caps="$(jq -er '.caps_applied' "$proof_evidence_resolved/receipt.json")"
proof_summary="receipt_sha256=$proof_receipt_sha attempt_id=$proof_attempt_id run_id=$proof_run_id auditor_sha256=$jankurai_sha proof_status=$proof_status score=$proof_score hard_findings=$proof_hard caps_applied=$proof_caps product_base_commit=$product_base_commit product_release_tag_ref=${product_release_tag_ref:-none} product_release_tag_commit=${product_release_tag_commit:-none} sibling_sources_sha256=${sibling_sources_sha:-none} cuda_compute_capability=${cuda_cap:-none} cuda_capability_record_sha256=${cuda_record_sha:-none} root_seal=$(jq -er '.root_seal' "$state")"
description="$required_check base=${product_base_commit:0:12} cuda-sm=${cuda_cap:-none} root-seal=$(jq -er '.root_seal' "$state" | cut -c1-16)"
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
