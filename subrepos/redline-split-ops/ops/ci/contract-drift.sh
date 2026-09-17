#!/usr/bin/env bash
set -euo pipefail

# shellcheck disable=SC1091,SC2154
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck disable=SC2154
cd "$repo_root"

mkdir -p target/contract-drift
"$repo_root/redlinectl" control-validate
readiness_receipt="target/contract-drift/readiness.json"
mirror="$repo_root/../redline.lock.toml"
mirror_sidecar="${mirror}.sha256"
if [[ ! -e "$mirror" && ! -L "$mirror" && ! -e "$mirror_sidecar" && ! -L "$mirror_sidecar" ]]; then
  "$repo_root/redlinectl" release-receipt --standalone "$readiness_receipt"
  expected_transition_state="standalone-authoritative"
  expected_cutover_eligible="false"
else
  "$repo_root/redlinectl" review-lock-verify
  "$repo_root/redlinectl" release-receipt "$readiness_receipt"
  expected_transition_state="synchronized"
  expected_cutover_eligible="true"
fi

head_sha="$(git rev-parse --verify 'HEAD^{commit}')"
manifest_sha="$(sha256sum repos.manifest.toml | awk '{print $1}')"
lock_path="$repo_root/redline.lock.toml"
lock_sha="$(sha256sum "$lock_path" | awk '{print $1}')"
transition_state="$(jq -er '.transition_state' "$readiness_receipt")"
cutover_eligible="$(jq -r '.cutover_eligible' "$readiness_receipt")"
[[ "$transition_state" == "$expected_transition_state" \
  && "$cutover_eligible" == "$expected_cutover_eligible" ]]

jq -n \
  --arg head_sha "$head_sha" \
  --arg manifest_sha256 "$manifest_sha" \
  --arg lock_sha256 "$lock_sha" \
  --arg transition_state "$transition_state" \
  --argjson cutover_eligible "$cutover_eligible" \
  '{
    schema_version:"redline.contract-drift/v1",
    status:"pass",
    authority:"redline-split-ops/repos.manifest.toml",
    head_sha:$head_sha,
    manifest_sha256:$manifest_sha256,
    authoritative_lock_sha256:$lock_sha256,
    transition_state:$transition_state,
    cutover_eligible:$cutover_eligible
  }' >target/contract-drift/receipt.json

printf 'Redline contract drift ok: Jain.6 authority state=%s cutover=%s\n' \
  "$transition_state" "$cutover_eligible"
