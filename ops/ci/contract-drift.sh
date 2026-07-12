#!/usr/bin/env bash
# Contract-drift lane: assert the OpenAPI contract, the Rust DTOs, and the
# TypeScript DTOs all agree. The contract surface (CONTRACT.md +
# contracts/openapi) is the public API; this is its drift check.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
ensure_artifacts

log "contract-drift: checking OpenAPI <-> Rust DTOs <-> TS DTOs"
if ! has jq; then
  missing_tool jq "contract drift receipt"
  exit 0
fi

openapi="contracts/openapi/redline-web.openapi.json"
rust_model="apps/api/src/model.rs"
ts_types="apps/web/src/api/types.ts"
human_contract="CONTRACT.md"
schemas_json="$(jq -ce '.components.schemas | keys' "$openapi")" \
  || fail "contract-drift: invalid OpenAPI schemas"
paths_json="$(jq -ce '.paths | keys' "$openapi")" \
  || fail "contract-drift: invalid OpenAPI paths"
missing=()

while IFS= read -r name; do
  if ! grep -Eq "(struct|enum)[[:space:]]+${name}([[:space:]<{]|$)" "$rust_model"; then
    missing+=("Rust model.rs missing type: ${name}")
  fi
  ts_name="$name"
  [[ "$name" == "ApiError" ]] && ts_name="ApiErrorBody"
  if ! grep -Eq "(interface|type)[[:space:]]+${ts_name}([[:space:]<{=]|$)" "$ts_types"; then
    missing+=("TS types.ts missing type: ${ts_name}")
  fi
done < <(jq -r '.[]' <<<"$schemas_json")

while IFS= read -r path; do
  token="${path//\{name\}/:name}"
  if ! grep -Fq -- "$path" "$human_contract" && ! grep -Fq -- "$token" "$human_contract"; then
    missing+=("CONTRACT.md missing endpoint: ${path}")
  fi
done < <(jq -r '.[]' <<<"$paths_json")

missing_json="$(json_array "${missing[@]}")"
ok=true
[[ "${#missing[@]}" -eq 0 ]] || ok=false
jq -n \
  --argjson ok "$ok" \
  --argjson schemas "$schemas_json" \
  --argjson paths "$paths_json" \
  --argjson drift "$missing_json" \
  '{
    ok: $ok,
    schemas_checked: $schemas,
    paths_checked: $paths,
    drift: $drift,
    sources: {
      openapi: "contracts/openapi/redline-web.openapi.json",
      rust: "apps/api/src/model.rs",
      typescript: "apps/web/src/api/types.ts",
      human: "CONTRACT.md"
    }
  }' >target/jankurai/contract-drift.json

if [[ "$ok" != true ]]; then
  printf '[contract-drift] DRIFT: %s\n' "${missing[@]}" >&2
  fail "contract drift detected"
fi
log "contract-drift: ok: $(jq 'length' <<<"$schemas_json") schemas, $(jq 'length' <<<"$paths_json") paths in sync"

log "contract-drift: complete"
