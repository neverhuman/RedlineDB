#!/usr/bin/env bash
# Cost-budget lane: assert the zero-spend manifest (redline-web runs entirely
# locally; there is no paid/external work) and emit a receipt.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
ensure_artifacts

log "cost-budget: validating zero-spend manifest"
python3 - <<'PY'
import json
from pathlib import Path
import tomllib

manifest_path = Path("agent/cost-budget.toml")
manifest = tomllib.loads(manifest_path.read_text())
for key, expected in {
    "default_external_spend_usd": 0,
    "default_network_spend_usd": 0,
}.items():
    if manifest.get(key) != expected:
        raise SystemExit(f"{key} must be {expected}")
quota_caps = manifest.get("quota_caps", {})
for key in ("external_api_usd", "model_api_usd"):
    if quota_caps.get(key) != 0:
        raise SystemExit(f"quota cap {key} must be zero by default")
stop_conditions = manifest.get("stop_conditions", {})
for key in ("on_missing_receipt", "on_unknown_paid_tool", "on_quota_exceeded", "on_kill_switch"):
    if stop_conditions.get(key) is not True:
        raise SystemExit(f"stop condition {key} must be true")
receipt = {
    "ok": True,
    "manifest": str(manifest_path),
    "default_external_spend_usd": manifest["default_external_spend_usd"],
    "quota_caps": quota_caps,
    "kill_switch_env": manifest["kill_switch_env"],
    "stop_conditions": stop_conditions,
}
Path("target/jankurai/cost-budget.json").write_text(json.dumps(receipt, indent=2) + "\n")
PY

log "cost-budget: complete"
