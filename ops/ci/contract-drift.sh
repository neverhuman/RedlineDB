#!/usr/bin/env bash
# Contract-drift lane: assert the OpenAPI contract, the Rust DTOs, and the
# TypeScript DTOs all agree. The contract surface (CONTRACT.md +
# contracts/openapi) is the public API; this is its drift check.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
ensure_artifacts

log "contract-drift: checking OpenAPI <-> Rust DTOs <-> TS DTOs"
python3 - <<'PY'
import json
import re
from pathlib import Path

openapi = json.loads(Path("contracts/openapi/redline-web.openapi.json").read_text())
schemas = openapi["components"]["schemas"]
model = Path("apps/api/src/model.rs").read_text()
types = Path("apps/web/src/api/types.ts").read_text()
contract = Path("CONTRACT.md").read_text()

# Rust struct/enum name == OpenAPI schema name, except where the wire envelope
# differs from the internal type name.
ts_alias = {"ApiError": "ApiErrorBody"}

missing = []
for name in schemas:
    rust_name = name
    if not re.search(rf"\b(struct|enum)\s+{re.escape(rust_name)}\b", model):
        missing.append(f"Rust model.rs missing type: {rust_name}")
    ts_name = ts_alias.get(name, name)
    if not re.search(rf"\b(interface|type)\s+{re.escape(ts_name)}\b", types):
        missing.append(f"TS types.ts missing type: {ts_name}")

# Every documented endpoint path in the OpenAPI must appear in CONTRACT.md.
for path in openapi["paths"]:
    token = path.replace("{name}", ":name")
    if path not in contract and token not in contract:
        missing.append(f"CONTRACT.md missing endpoint: {path}")

receipt = {
    "ok": not missing,
    "schemas_checked": sorted(schemas.keys()),
    "paths_checked": sorted(openapi["paths"].keys()),
    "drift": missing,
    "sources": {
        "openapi": "contracts/openapi/redline-web.openapi.json",
        "rust": "apps/api/src/model.rs",
        "typescript": "apps/web/src/api/types.ts",
        "human": "CONTRACT.md",
    },
}
Path("target/jankurai/contract-drift.json").write_text(json.dumps(receipt, indent=2) + "\n")
if missing:
    for m in missing:
        print(f"[contract-drift] DRIFT: {m}")
    raise SystemExit("contract drift detected")
print(f"[contract-drift] ok: {len(schemas)} schemas, {len(openapi['paths'])} paths in sync")
PY

log "contract-drift: complete"
