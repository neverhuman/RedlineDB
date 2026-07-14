#!/usr/bin/env bash
set -euo pipefail

ops_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
manifest="${ops_root}/repos.manifest.toml"
base="${JERYU_BASE:-http://127.0.0.1:8787}"
family_filter=""
check_only=0
receipt=""
rows=()

usage() {
  printf 'usage: %s [--manifest PATH] [--base URL] [--family NAME] [--check-only] [--receipt PATH]\n' "$0" >&2
}

jeryu_token() {
  if [[ -n "${JERYU_MERGE_TOKEN:-}" ]]; then
    printf '%s' "$JERYU_MERGE_TOKEN"
    return
  fi
  local file="${JERYU_MERGE_TOKEN_FILE:-$HOME/.jeryu/secrets/merge-token}"
  [[ -r "$file" ]] && tr -d '\n' <"$file"
}

emit_receipt() {
  local rc="$?" status=pass mode=apply repositories='[]'
  [[ "$rc" -eq 0 ]] || status=fail
  [[ "$check_only" == "1" ]] && mode=check-only
  if (( ${#rows[@]} > 0 )); then
    repositories="$(printf '%s\n' "${rows[@]}" | jq -Rsc '
      split("\n") | map(select(length > 0) | split("|") |
      {family:.[0],owner:.[1],name:.[2]})')"
  fi
  mkdir -p "$(dirname "$receipt")"
  jq -n --arg schema_version 'jain.forge-family-registration/v1' \
    --arg status "$status" --arg mode "$mode" --arg manifest "$manifest" \
    --arg base "$base" --arg generated_at "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" \
    --argjson repositories "$repositories" \
    '{schema_version:$schema_version,status:$status,mode:$mode,manifest:$manifest,forge:$base,repositories:$repositories,generated_at:$generated_at}' \
    >"$receipt"
  return "$rc"
}
while [[ $# -gt 0 ]]; do
  case "$1" in
    --manifest) shift; manifest="${1:-}" ;;
    --base) shift; base="${1:-}" ;;
    --family) shift; family_filter="${1:-}" ;;
    --check-only) check_only=1 ;;
    --receipt) shift; receipt="${1:-}" ;;
    *) usage; exit 2 ;;
  esac
  shift
done

[[ -r "$manifest" ]] || { printf 'manifest not readable: %s\n' "$manifest" >&2; exit 1; }
release_version="$(awk -F'"' '/^release_version = / {print $2; exit}' "$manifest")"
[[ "$release_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  printf 'manifest release_version is missing or invalid: %s\n' "$manifest" >&2
  exit 1
}
receipt="${receipt:-${ops_root}/docs/release-evidence/${release_version}/forge-family-registration.json}"
trap emit_receipt EXIT
for tool in cargo curl jq; do
  command -v "$tool" >/dev/null 2>&1 || { printf 'required tool missing: %s\n' "$tool" >&2; exit 1; }
done

managed_json="$(cargo run --locked --quiet --manifest-path "$ops_root/Cargo.toml" -- \
  managed-repos --manifest "$manifest" --json)"
mapfile -t rows < <(jq -r --arg family "$family_filter" '
  .repositories[]
  | select(.family_registered == true)
  | select($family == "" or .family == $family)
  | (.remote | sub("^.*/git/"; "") | sub("\\.git$"; "")) as $slug
  | select($slug | contains("/"))
  | [.family, ($slug | split("/")[0]), ($slug | split("/")[1])]
  | join("|")
' <<<"$managed_json")
(( ${#rows[@]} > 0 )) || { printf 'managed repository set is empty\n' >&2; exit 1; }

token="$(jeryu_token)"
auth_args=()
if [[ "$check_only" != "1" ]]; then
  [[ -n "$token" ]] || { printf 'local forge write credential is unavailable\n' >&2; exit 1; }
  auth_args=(-H "Authorization: Bearer $token")
  for row in "${rows[@]}"; do
    IFS='|' read -r family owner name <<<"$row"
    curl -fsS -X PATCH "$base/api/v1/repos/${owner}%2F${name}" \
      "${auth_args[@]}" -H 'content-type: application/json' \
      -d "$(jq -cn --arg family "$family" '{family:$family}')" >/dev/null
    printf 'registered %s/%s -> %s\n' "$owner" "$name" "$family"
  done
elif [[ -n "$token" ]]; then
  auth_args=(-H "Authorization: Bearer $token")
fi

repos_json="$(curl -fsS "${auth_args[@]}" "$base/api/v1/repos?host=jeryu")"
for row in "${rows[@]}"; do
  IFS='|' read -r family owner name <<<"$row"
  jq -e --arg owner "$owner" --arg name "$name" --arg family "$family" '
    [.repositories[]? | select(.id.owner == $owner and .id.name == $name and .family == $family)]
    | length == 1
  ' <<<"$repos_json" >/dev/null || {
    printf 'family mismatch or missing repo: %s/%s -> %s\n' "$owner" "$name" "$family" >&2
    exit 1
  }
done
mapfile -t families < <(printf '%s\n' "${rows[@]}" | cut -d'|' -f1 | sort -u)
for family in "${families[@]}"; do
  jq -e --arg family "$family" '.facets.families // [] | any(. == $family)' \
    <<<"$repos_json" >/dev/null || { printf 'facets missing family: %s\n' "$family" >&2; exit 1; }
done
printf 'verified %s managed repositories across %s forge families\n' "${#rows[@]}" "${#families[@]}"
