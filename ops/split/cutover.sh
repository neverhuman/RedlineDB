#!/usr/bin/env bash
# Read-only local cutover validation for the active release candidate.
set -euo pipefail

split="${JAIN_SPLIT_ROOT:-/home/ubuntu/jain-split}"
ops_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
deploy="${split}/jain-deploy"
bin_src="${JAIN_BIN_SRC:-${deploy}/target/release/jain}"
manifest="${JAIN_SPLIT_MANIFEST:-${ops_root}/repos.manifest.toml}"
deploy_lock="${JAIN_DEPLOY_LOCK:-${deploy}/jain-split.lock.toml}"
[[ -r "$manifest" ]] || { printf 'manifest not readable: %s\n' "$manifest" >&2; exit 1; }
release_version="$(awk -F'"' '/^release_version = / {print $2; exit}' "$manifest")"
[[ "$release_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  printf 'manifest release_version is missing or invalid: %s\n' "$manifest" >&2
  exit 1
}
receipt="${ops_root}/docs/release-evidence/${release_version}/local-cutover-dry-run.json"
dry_run=0

usage() {
  printf 'usage: %s --dry-run [--receipt PATH]\n' "$0" >&2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) dry_run=1 ;;
    --receipt)
      shift
      receipt="${1:-}"
      ;;
    *) usage; exit 2 ;;
  esac
  shift
done

[[ "$dry_run" == "1" ]] || {
  printf 'v8 production/local installation writes are out of scope; pass --dry-run\n' >&2
  exit 2
}
[[ -n "$receipt" ]] || { usage; exit 2; }
command -v jq >/dev/null 2>&1 || { printf 'jq is required\n' >&2; exit 1; }
command -v sha256sum >/dev/null 2>&1 || { printf 'sha256sum is required\n' >&2; exit 1; }
[[ -x "$bin_src" ]] || { printf 'missing release binary: %s\n' "$bin_src" >&2; exit 1; }
version="$($bin_src --version)"
release_pattern="${release_version//./\\.}"
grep -Eq "(^|[[:space:]])${release_pattern}([[:space:]]|$)" <<<"$version" || {
  printf 'release binary does not report product version %s: %s\n' "$release_version" "$version" >&2
  exit 1
}
[[ -f "$deploy_lock" ]] || { printf 'missing deploy lock: %s\n' "$deploy_lock" >&2; exit 1; }

splitctl="${JAIN_SPLITCTL:-}"
if [[ -z "$splitctl" ]]; then
  for candidate in "${ops_root}/target/release/splitctl" "${ops_root}/target/debug/splitctl"; do
    if [[ -x "$candidate" ]]; then
      splitctl="$candidate"
      break
    fi
  done
fi
[[ -n "$splitctl" && -x "$splitctl" ]] || {
  printf 'splitctl is required; build it or set JAIN_SPLITCTL to the reviewed binary\n' >&2
  exit 1
}
"$splitctl" validate-deploy-lock --manifest "$manifest" --lock "$deploy_lock"

authority_sha256="$(sha256sum "$manifest" | awk '{print $1}')"
deploy_lock_sha256="$(sha256sum "$deploy_lock" | awk '{print $1}')"
redline_tag="$(awk '
  /^\[nested\.redline\]$/ { in_redline=1; next }
  /^\[/ { in_redline=0 }
  in_redline && /^engine_tag[[:space:]]*=/ { split($0, parts, "\""); print parts[2]; exit }
' "$deploy_lock")"
redline_commit="$(awk '
  /^\[nested\.redline\]$/ { in_redline=1; next }
  /^\[/ { in_redline=0 }
  in_redline && /^engine_commit[[:space:]]*=/ { split($0, parts, "\""); print parts[2]; exit }
' "$deploy_lock")"

printf '[dry-run] would install %q into the operator-selected local prefix\n' "$bin_src"
printf '[dry-run] no systemd unit, process, route, alias, or production state was changed\n'
mkdir -p "$(dirname "$receipt")"
jq -n \
  --arg schema_version 'jain.cutover.dry-run/v1' \
  --arg release "$release_version" --arg status 'pass' --arg binary "$bin_src" \
  --arg version_output "$version" --arg rollback_target '7.0.6' \
  --arg authority_manifest "$manifest" --arg authority_sha256 "$authority_sha256" \
  --arg deploy_lock "$deploy_lock" --arg deploy_lock_sha256 "$deploy_lock_sha256" \
  --arg redline_tag "$redline_tag" --arg redline_commit "$redline_commit" \
  --arg generated_at "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" \
  '{schema_version:$schema_version,release:$release,mode:"dry-run",status:$status,binary:$binary,version_output:$version_output,authority:{manifest:$authority_manifest,sha256:$authority_sha256},deploy_lock:{path:$deploy_lock,sha256:$deploy_lock_sha256},redline:{tag:$redline_tag,commit:$redline_commit},rollback_target:$rollback_target,external_mutations:[],generated_at:$generated_at}' \
  >"$receipt"
printf 'wrote %s\n' "$receipt"
