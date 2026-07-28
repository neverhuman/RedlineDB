#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/host-ci-inputs.sh
source "$repo_root/ops/ci/host-ci-inputs.sh"
mkdir -p "$repo_root/target/test-tmp"
tmp="$(mktemp -d "$repo_root/target/test-tmp/host-ci-inputs.XXXXXX")"
cleanup() {
  chmod -R u+rwX "$tmp" 2>/dev/null || true
  rm -rf -- "$tmp"
}
trap cleanup EXIT

uid="$(id -u)"
gid="$(id -g)"
wheelhouse="$tmp/wheelhouse"
mkdir -m 0755 "$wheelhouse"
printf 'wheel-a\n' >"$wheelhouse/alpha-1.0-py3-none-any.whl"
printf 'wheel-b\n' >"$wheelhouse/bravo-2.0-py3-none-any.whl"
chmod 0444 "$wheelhouse"/*.whl
chmod 0555 "$wheelhouse"
inventory="$(jain_host_ci_python_wheelhouse_inventory "$wheelhouse" "$uid" "$gid")"
inventory_sha="${inventory%%$'\t'*}"
[[ "$inventory_sha" =~ ^[0-9a-f]{64}$ ]]
jain_host_ci_verify_python_wheelhouse \
  "$wheelhouse" "$inventory_sha" "$uid" "$gid"

chmod 0755 "$wheelhouse"
ln "$wheelhouse/alpha-1.0-py3-none-any.whl" \
  "$wheelhouse/linked-1.0-py3-none-any.whl"
chmod 0555 "$wheelhouse"
if jain_host_ci_python_wheelhouse_inventory \
    "$wheelhouse" "$uid" "$gid" >/dev/null; then
  printf 'wheelhouse validator accepted a hard-linked wheel\n' >&2
  exit 1
fi
chmod 0755 "$wheelhouse"
rm -f -- "$wheelhouse/linked-1.0-py3-none-any.whl"
chmod 0555 "$wheelhouse"

source_root="$tmp/source"
staging_root="$tmp/staging"
mkdir -p "$source_root/target/jankurai/coverage" \
  "$source_root/target/security" "$staging_root"
printf '{"status":"pass"}\n' \
  >"$source_root/target/jankurai/coverage/coverage-audit.json"
printf '{"status":"pass"}\n' >"$source_root/target/security/evidence.json"
chmod 0644 \
  "$source_root/target/jankurai/coverage/coverage-audit.json"
chmod 0600 "$source_root/target/security/evidence.json"
request_id="$(printf 'a%.0s' {1..64})"
control_commit="$(printf 'b%.0s' {1..40})"
head_sha="$(printf 'c%.0s' {1..40})"
jain_host_ci_stage_audit_inputs \
  "$source_root" "$staging_root" "$request_id" "$control_commit" \
  veox jain-python "$head_sha" jain-python/required
validation="$(jain_host_ci_validate_audit_inputs \
  "$staging_root" "$request_id" "$control_commit" \
  veox jain-python "$head_sha" jain-python/required "$uid" "$gid")"
[[ "${validation##*$'\t'}" == 2 ]]

printf 'unexpected\n' >"$staging_root/files/unexpected"
chmod 0600 "$staging_root/files/unexpected"
if jain_host_ci_validate_audit_inputs \
    "$staging_root" "$request_id" "$control_commit" \
    veox jain-python "$head_sha" jain-python/required "$uid" "$gid" \
    >/dev/null; then
  printf 'audit-input validator accepted an undeclared file\n' >&2
  exit 1
fi
rm -- "$staging_root/files/unexpected"

mkdir -m 0700 "$staging_root/files/unexpected-empty-directory"
if jain_host_ci_validate_audit_inputs \
    "$staging_root" "$request_id" "$control_commit" \
    veox jain-python "$head_sha" jain-python/required "$uid" "$gid" \
    >/dev/null; then
  printf 'audit-input validator accepted an undeclared directory\n' >&2
  exit 1
fi
rmdir -- "$staging_root/files/unexpected-empty-directory"

jq '.repository="jain-report"' "$staging_root/receipt.json" \
  >"$staging_root/receipt.tampered"
chmod 0600 "$staging_root/receipt.tampered"
mv -- "$staging_root/receipt.tampered" "$staging_root/receipt.json"
if jain_host_ci_validate_audit_inputs \
    "$staging_root" "$request_id" "$control_commit" \
    veox jain-python "$head_sha" jain-python/required "$uid" "$gid" \
    >/dev/null; then
  printf 'audit-input validator accepted a mismatched repository binding\n' >&2
  exit 1
fi

printf 'host CI input validation passed\n'
