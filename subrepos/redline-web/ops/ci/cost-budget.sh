#!/usr/bin/env bash
# Cost-budget lane: assert the zero-spend manifest (redline-web runs entirely
# locally; there is no paid/external work) and emit a receipt.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
ensure_artifacts

log "cost-budget: validating zero-spend manifest"
if ! has jq; then
  missing_tool jq "cost budget receipt"
  exit 0
fi

manifest="agent/cost-budget.toml"
for assignment in \
  'default_external_spend_usd[[:space:]]*=[[:space:]]*0' \
  'default_network_spend_usd[[:space:]]*=[[:space:]]*0' \
  'external_api_usd[[:space:]]*=[[:space:]]*0' \
  'model_api_usd[[:space:]]*=[[:space:]]*0'; do
  grep -Eq "^${assignment}[[:space:]]*$" "$manifest" \
    || fail "cost-budget: expected zero assignment matching ${assignment}"
done
for key in on_missing_receipt on_unknown_paid_tool on_quota_exceeded on_kill_switch; do
  grep -Eq "^${key}[[:space:]]*=[[:space:]]*true[[:space:]]*$" "$manifest" \
    || fail "cost-budget: stop condition ${key} must be true"
done
kill_switch_env="$(sed -nE 's/^kill_switch_env[[:space:]]*=[[:space:]]*"([^"]+)"[[:space:]]*$/\1/p' "$manifest")"
[[ -n "$kill_switch_env" ]] || fail "cost-budget: kill_switch_env is missing"

jq -n --arg manifest "$manifest" --arg kill_switch_env "$kill_switch_env" '{
  ok: true,
  manifest: $manifest,
  default_external_spend_usd: 0,
  quota_caps: {external_api_usd: 0, model_api_usd: 0},
  kill_switch_env: $kill_switch_env,
  stop_conditions: {
    on_missing_receipt: true,
    on_unknown_paid_tool: true,
    on_quota_exceeded: true,
    on_kill_switch: true
  }
}' >target/jankurai/cost-budget.json

log "cost-budget: complete"
