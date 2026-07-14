#!/usr/bin/env bash
# WQ-9 — Jain v8.0.0 E2E studies harness (receipt schema: jain.e2e-studies/v1)
#
# Drives the four owner-required end-to-end studies against a LIVE jain-web instance
# and emits a signed-shape receipt. curl + jq only (no Playwright). The endpoint/assert
# contract was verified against the real feat-web binary (see WQ-9 recon).
#
#   (a) upload    — create session, upload CSV (role=train FIRST, then file), see upload events
#   (b) training  — drive to a terminal-success state; observe the algorithm-search engine
#   (c) chimera   — chimera_enabled at low + a completed `starforge` classification trial
#   (d) export    — algorithm export zip carries only shareable source, zero restricted content
#
# Usage:
#   bin/e2e-studies.sh                          # fast low-effort run vs 127.0.0.1:4180
#   BASE_URL=http://host:port EFFORT=medium EXPECT_ENGINE=prime ENGINE_STRICT=1 bin/e2e-studies.sh
#
# Env knobs:
#   BASE_URL (http://127.0.0.1:4180)  EFFORT (low)  AUTH_KEY ("")  DATASET (data/e2e.csv)
#   EXPECT_ENGINE (legacy|lime|prime|any)  ENGINE_STRICT (0)  — gate verdict on engine match
#   POLL_TIMEOUT (1800s / 30-min cap)  POLL_INTERVAL (3s)  MODE (fast)
#
# Exit: 0 iff verdict==pass. Receipt + evidence under receipts/<run_id>/.
set -uo pipefail

# ------------------------------------------------------------------ config
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"          # the e2e-studies/ dir
BASE_URL="${BASE_URL:-http://127.0.0.1:4180}"; BASE_URL="${BASE_URL%/}"
EFFORT="${EFFORT:-low}"
EXPECT_ENGINE="${EXPECT_ENGINE:-legacy}"                          # legacy|lime|prime|any
ENGINE_STRICT="${ENGINE_STRICT:-0}"                              # 1 => engine mismatch fails verdict
AUTH_KEY="${AUTH_KEY:-}"
MODE="${MODE:-fast}"
DATASET="${DATASET:-$HERE/data/e2e.csv}"
POLL_TIMEOUT="${POLL_TIMEOUT:-1800}"
POLL_INTERVAL="${POLL_INTERVAL:-3}"
GEN_AWK="$HERE/bin/gen-dataset.awk"

# Fast low-effort overrides (every duration knob shrunk; invent_gens>0 is REQUIRED so the
# invention phase runs and the /export/algorithm study has an invention artifact to export).
# chimera_enabled is intentionally NOT overridden (defaults true at low -> study C assert).
OVERRIDES_FAST='{"invent_gens":6,"invent_pop":16,"gp_gens":12,"cv_iters":6,"ho_iters":8,"gp_pop":32,"max_gp":32,"hyperion_sweeps":1,"eda_pop":8,"eda_gens":2,"p2_iters":4}'
OVERRIDES="${OVERRIDES:-$OVERRIDES_FAST}"

RESTRICTED_RE='chimera|hyperion|starforge|weight|embedding|context|bundle|\.safetensors|\.pt|\.onnx'

RUN_ID="run-$(date -u +%Y%m%dT%H%M%SZ)-$$"
OUT="$HERE/receipts/$RUN_ID"
mkdir -p "$OUT"
GENERATED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

log() { printf '  %s\n' "$*" >&2; }
hr()  { printf '%s\n' "----------------------------------------------------------------" >&2; }

# curl wrappers: write body to $1, echo HTTP status. Auth header only if AUTH_KEY set.
AUTH_ARGS=(); [ -n "$AUTH_KEY" ] && AUTH_ARGS=(-H "authorization: Bearer $AUTH_KEY")
api_get()   { curl -sS --max-time 20  "${AUTH_ARGS[@]}" -o "$1" -w '%{http_code}' "$BASE_URL$2"; }
api_postj() { curl -sS --max-time 60  "${AUTH_ARGS[@]}" -o "$1" -w '%{http_code}' -H 'content-type: application/json' -X POST --data "$3" "$BASE_URL$2"; }
api_poste() { curl -sS --max-time 60  "${AUTH_ARGS[@]}" -o "$1" -w '%{http_code}' -X POST "$BASE_URL$2"; }
api_dl()    { curl -sS --max-time 120 "${AUTH_ARGS[@]}" -o "$1" -w '%{http_code}' "$BASE_URL$2"; }

# study result accumulators (globals; read by write_receipt)
UP_STATUS=fail; TR_STATUS=fail; CH_STATUS=fail; EX_STATUS=fail
ENGINE_OBSERVED=unknown; TERMINAL_STATE=unknown; STARFORGE_TRIALS=0; ENGINE_MATCH=true
UP_DETAIL=""; TR_DETAIL=""; CH_DETAIL=""; EX_DETAIL=""; VERDICT=fail
CHIMERA_ENABLED=false; SESSION_ID=""; MODEL_ID=""
DATASET_SHA=""; DATASET_ROWS=0; RUNNER=unknown

write_receipt() {   # assemble receipt.json + sidecar from current globals; sets VERDICT
  local verdict=pass
  for s in "$UP_STATUS" "$TR_STATUS" "$CH_STATUS" "$EX_STATUS"; do [ "$s" = pass ] || verdict=fail; done
  VERDICT="$verdict"
  jq -n \
    --arg schema "jain.e2e-studies/v1" \
    --arg run_id "$RUN_ID" --arg mode "$MODE" --arg base "$BASE_URL" \
    --arg effort "$EFFORT" --arg runner "$RUNNER" --arg gen "$GENERATED_AT" \
    --arg ds_path "docs/release-evidence/8.0.0/e2e-studies/data/e2e.csv" \
    --arg ds_sha "$DATASET_SHA" --argjson ds_rows "${DATASET_ROWS:-0}" \
    --arg sid "$SESSION_ID" --argjson chimera "$([ "$CHIMERA_ENABLED" = true ] && echo true || echo false)" \
    --arg up "$UP_STATUS" --arg tr "$TR_STATUS" --arg ch "$CH_STATUS" --arg ex "$EX_STATUS" \
    --arg up_d "$UP_DETAIL" --arg tr_d "$TR_DETAIL" --arg ch_d "$CH_DETAIL" --arg ex_d "$EX_DETAIL" \
    --arg engine "$ENGINE_OBSERVED" --arg expect "$EXPECT_ENGINE" \
    --argjson match "$([ "${ENGINE_MATCH:-true}" = true ] && echo true || echo false)" \
    --arg terminal "$TERMINAL_STATE" --argjson starforge "${STARFORGE_TRIALS:-0}" \
    --arg model_id "$MODEL_ID" \
    --arg exc "docs/release-evidence/8.0.0/jankurai-auditor-exception.json" \
    --arg verdict "$verdict" \
    '{schema:$schema, run_id:$run_id, generated_at:$gen, mode:$mode, base_url:$base,
      effort:$effort, runner:$runner, auditor_exception:$exc,
      dataset:{path:$ds_path, sha256:$ds_sha, rows:$ds_rows},
      session:{id:$sid, chimera_enabled:$chimera, model_id:$model_id, terminal_state:$terminal},
      engine_observed:$engine, engine_expected:$expect, engine_match:$match,
      studies:{
        upload:{status:$up, detail:$up_d},
        training:{status:$tr, engine_observed:$engine, terminal_state:$terminal, detail:$tr_d},
        chimera:{status:$ch, starforge_completed_trials:$starforge, detail:$ch_d},
        export:{status:$ex, detail:$ex_d}
      },
      verdict:$verdict}' > "$OUT/receipt.json"
  ( cd "$OUT" && sha256sum receipt.json > receipt.json.sha256 )
}

fail_receipt() { write_receipt; log "verdict=FAIL (early exit)"; exit 1; }

# --------------------------------------------------------------- preflight
hr; log "WQ-9 E2E studies  run_id=$RUN_ID  base=$BASE_URL  effort=$EFFORT  mode=$MODE"; hr
[ -f "$DATASET" ] || { awk -f "$GEN_AWK" > "$DATASET" 2>"$OUT/gen.log" && log "generated dataset $DATASET"; }
DATASET_SHA="$(sha256sum "$DATASET" | awk '{print $1}')"
DATASET_ROWS="$(( $(awk 'END{print NR}' "$DATASET") - 1 ))"
cp "$DATASET" "$OUT/dataset.csv"
log "dataset: rows=$DATASET_ROWS sha256=$DATASET_SHA"

st=$(api_get "$OUT/health.json" "/api/health")
[ "$st" = "200" ] || log "WARN: /api/health -> $st (server not up?)"
api_get "$OUT/system.json" "/api/system" >/dev/null
RUNNER="$(jq -r '.runtime.runner // "unknown"' "$OUT/system.json" 2>/dev/null)"
log "health=$st runner=$RUNNER"

# =========================================================== STUDY A: upload
hr; log "STUDY (a) upload"
body='{"effort":"'"$EFFORT"'","overrides":'"$OVERRIDES"'}'
st=$(api_postj "$OUT/session-create.json" "/api/sessions" "$body")
if [ "$st" != "200" ]; then UP_DETAIL="create HTTP $st"; log "create session -> $st"; fail_receipt; fi
SESSION_ID="$(jq -r '.id' "$OUT/session-create.json")"
CHIMERA_ENABLED="$(jq -r '.resolved_knobs.chimera_enabled' "$OUT/session-create.json")"
jq '.resolved_knobs' "$OUT/session-create.json" > "$OUT/resolved_knobs.json" 2>/dev/null
log "session=$SESSION_ID chimera_enabled=$CHIMERA_ENABLED"

# Upload: role=train part FIRST, then the file (filename MUST end .csv). File part name is not
# validated by the server, but WQ-9 uses "file".
st=$(curl -sS --max-time 120 "${AUTH_ARGS[@]}" -o "$OUT/upload.json" -w '%{http_code}' \
      -F 'role=train' \
      -F "file=@$DATASET;type=text/csv;filename=e2e.csv" \
      -X POST "$BASE_URL/api/sessions/$SESSION_ID/files")
UPLOAD_FILE_STATUS="$(jq -r '.files[0].status // "none"' "$OUT/upload.json" 2>/dev/null)"
POST_UPLOAD_STATE="$(jq -r '.state // "unknown"' "$OUT/upload.json" 2>/dev/null)"
log "upload HTTP $st  files[0].status=$UPLOAD_FILE_STATUS  state=$POST_UPLOAD_STATE"

api_get "$OUT/events-upload.json" "/api/sessions/$SESSION_ID/events?after=0" >/dev/null
HAS_UP_START="$(jq -r '[.[]|select(.kind=="upload.started")]|length' "$OUT/events-upload.json" 2>/dev/null)"
HAS_UP_DONE="$(jq -r '[.[]|select(.kind=="upload.complete")]|length' "$OUT/events-upload.json" 2>/dev/null)"
if [ "$st" = "200" ] && [ "$UPLOAD_FILE_STATUS" = "uploaded" ] && [ "${HAS_UP_START:-0}" -ge 1 ] && [ "${HAS_UP_DONE:-0}" -ge 1 ]; then
  UP_STATUS=pass
fi
UP_DETAIL="http=$st file_status=$UPLOAD_FILE_STATUS upload.started=$HAS_UP_START upload.complete=$HAS_UP_DONE"
log "STUDY (a) upload => $UP_STATUS  [$UP_DETAIL]"

# ================================================= drive training to terminal
hr; log "driving training (auto-start aware)"
get_state() { api_get "$OUT/_state.json" "/api/sessions/$SESSION_ID" >/dev/null; jq -r '.state // "unknown"' "$OUT/_state.json"; }
started_kick=false
for i in $(seq 1 20); do
  cs="$(get_state)"
  case "$cs" in
    awaiting_target)
      log "state=awaiting_target -> approving target 'target'"
      api_postj "$OUT/target.json" "/api/sessions/$SESSION_ID/target" '{"target":"target","approved":true}' >/dev/null ;;
    ready_to_train)
      if [ "$started_kick" = false ]; then log "state=ready_to_train -> POST /start"; api_poste "$OUT/start.json" "/api/sessions/$SESSION_ID/start" >/dev/null; started_kick=true; fi ;;
    training|complete|prediction_ready|failed|cancelled)
      log "state=$cs -> training underway/terminal"; break ;;
    *) : ;;   # created/uploading/uploaded/profiling: settle, retry
  esac
  sleep 2
done

deadline=$(( $(date +%s) + POLL_TIMEOUT ))
while :; do
  TERMINAL_STATE="$(get_state)"
  case "$TERMINAL_STATE" in
    complete|prediction_ready) log "training terminal: $TERMINAL_STATE"; break ;;
    failed|cancelled)          log "training terminal FAIL: $TERMINAL_STATE"; break ;;
  esac
  [ "$(date +%s)" -ge "$deadline" ] && { log "training poll TIMEOUT after ${POLL_TIMEOUT}s (state=$TERMINAL_STATE)"; break; }
  sleep "$POLL_INTERVAL"
done

# ===================================================== STUDY B: training/engine
hr; log "STUDY (b) training + engine observation"
api_get "$OUT/events.json" "/api/sessions/$SESSION_ID/events?after=0" >/dev/null
P6_DONE="$(jq -r '[.[]|select(.kind=="training.progress" and (.payload.phase==6) and (.payload.event=="done"))]|length' "$OUT/events.json" 2>/dev/null)"
NOTE6="$(jq -r '[.[]|select(.kind=="training.progress" and (.payload.phase==6) and (.payload.event=="done"))|.payload.note // empty]|.[0] // ""' "$OUT/events.json" 2>/dev/null)"
RNOTES="$(jq -r '[.[]|select(.kind=="reasoning.note")|.message]|join(" | ")' "$OUT/events.json" 2>/dev/null)"
ENGINE_BLOB="$NOTE6 $RNOTES"
if   printf '%s' "$ENGINE_BLOB" | grep -qi 'Lime';  then ENGINE_OBSERVED=lime
elif printf '%s' "$ENGINE_BLOB" | grep -qi 'Prime'; then ENGINE_OBSERVED=prime
else ENGINE_OBSERVED=legacy; fi
case "$TERMINAL_STATE" in complete|prediction_ready) TR_STATUS=pass ;; *) TR_STATUS=fail ;; esac
ENGINE_MATCH=true
if [ "$EXPECT_ENGINE" != "any" ] && [ "$ENGINE_OBSERVED" != "$EXPECT_ENGINE" ]; then ENGINE_MATCH=false; fi
if [ "$ENGINE_STRICT" = "1" ] && [ "$ENGINE_MATCH" = false ]; then TR_STATUS=fail; fi
TR_DETAIL="terminal=$TERMINAL_STATE phase6_done=$P6_DONE engine=$ENGINE_OBSERVED expect=$EXPECT_ENGINE match=$ENGINE_MATCH strict=$ENGINE_STRICT"
log "STUDY (b) training => $TR_STATUS  [$TR_DETAIL]"

# ===================================================== STUDY C: chimera/starforge
hr; log "STUDY (c) chimera classification (starforge trial)"
if [ "$TR_STATUS" = pass ]; then
  api_get "$OUT/session-final.json" "/api/sessions/$SESSION_ID" >/dev/null
  MODEL_ID="$(jq -r '[.artifacts[]|select(.kind=="model")][0].id // empty' "$OUT/session-final.json" 2>/dev/null)"
  [ -z "$MODEL_ID" ] && MODEL_ID="$(jq -r '[.[]|select(.kind=="artifact.ready" and (.payload.kind=="model"))][0].payload.artifact_id // empty' "$OUT/events.json" 2>/dev/null)"
  log "model_id=$MODEL_ID"
  if [ -n "$MODEL_ID" ]; then
    st=$(api_dl "$OUT/model.zip" "/api/artifacts/$MODEL_ID/download")
    if [ "$st" = "200" ] && unzip -p "$OUT/model.zip" manifest.json > "$OUT/model-manifest.json" 2>/dev/null; then
      STARFORGE_TRIALS="$(jq -r '[.run.model_trials[]?|select(.backend=="starforge" and .status=="completed")]|length' "$OUT/model-manifest.json" 2>/dev/null)"
      STARFORGE_TRIALS="${STARFORGE_TRIALS:-0}"
      [ "$CHIMERA_ENABLED" = "true" ] && [ "${STARFORGE_TRIALS:-0}" -ge 1 ] && CH_STATUS=pass
      CH_DETAIL="dl=$st chimera_enabled=$CHIMERA_ENABLED starforge_completed_trials=$STARFORGE_TRIALS"
    else CH_DETAIL="model download HTTP $st or manifest.json missing"; fi
  else CH_DETAIL="no model artifact id found"; fi
else CH_DETAIL="skipped (training did not complete)"; fi
log "STUDY (c) chimera => $CH_STATUS  [$CH_DETAIL]"

# ===================================================== STUDY D: algorithm export
hr; log "STUDY (d) algorithm export (leak-free)"
if [ "$TR_STATUS" = pass ]; then
  st=$(api_dl "$OUT/export.zip" "/api/sessions/$SESSION_ID/export/algorithm")
  if [ "$st" = "200" ] && unzip -Z1 "$OUT/export.zip" > "$OUT/export-entries.txt" 2>/dev/null; then
    HAS_README=$(grep -c '^README\.md$' "$OUT/export-entries.txt")
    HAS_MANIFEST=$(grep -c '^MANIFEST\.txt$' "$OUT/export-entries.txt")
    HAS_PY=$(grep -cE '^invention.*/model\.py$' "$OUT/export-entries.txt")
    HAS_RS=$(grep -cE '^invention.*/model\.rs$' "$OUT/export-entries.txt")
    HAS_DSL=$(grep -cE '^invention.*/genome\.dsl$' "$OUT/export-entries.txt")
    RESTRICTED_HITS=$(grep -icE "$RESTRICTED_RE" "$OUT/export-entries.txt")
    if [ "$HAS_README" -ge 1 ] && [ "$HAS_MANIFEST" -ge 1 ] && [ "$HAS_PY" -ge 1 ] && [ "$HAS_RS" -ge 1 ] && [ "$HAS_DSL" -ge 1 ] && [ "$RESTRICTED_HITS" -eq 0 ]; then
      EX_STATUS=pass
    fi
    EX_DETAIL="dl=$st readme=$HAS_README manifest=$HAS_MANIFEST py=$HAS_PY rs=$HAS_RS dsl=$HAS_DSL restricted_hits=$RESTRICTED_HITS"
  else EX_DETAIL="export download HTTP $st or unzip failed"; fi
else EX_DETAIL="skipped (training did not complete)"; fi
log "STUDY (d) export => $EX_STATUS  [$EX_DETAIL]"

# =============================================================== receipt
write_receipt
hr
log "RECEIPT: $OUT/receipt.json"
jq . "$OUT/receipt.json" >&2
hr
jq -c '{run_id,verdict,engine_observed,studies:{upload:.studies.upload.status,training:.studies.training.status,chimera:.studies.chimera.status,export:.studies.export.status}}' "$OUT/receipt.json"

[ "$VERDICT" = pass ] && exit 0 || exit 1
