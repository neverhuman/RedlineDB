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
set -euo pipefail

# ------------------------------------------------------------------ config
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"          # the e2e-studies/ dir
BASE_URL="${BASE_URL:-http://127.0.0.1:4180}"; BASE_URL="${BASE_URL%/}"
EFFORT="${EFFORT:-low}"
EXPECT_ENGINE="${EXPECT_ENGINE:-legacy}"                          # legacy|lime|prime|any
ENGINE_STRICT="${ENGINE_STRICT:-1}"                              # 1 => engine mismatch fails verdict
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

case "$POLL_TIMEOUT" in ''|*[!0-9]*) printf 'POLL_TIMEOUT must be an integer\n' >&2; exit 2 ;; esac
case "$POLL_INTERVAL" in ''|*[!0-9.]*) printf 'POLL_INTERVAL must be numeric\n' >&2; exit 2 ;; esac
[ "$POLL_TIMEOUT" -le 1800 ] || { printf 'POLL_TIMEOUT exceeds the 30-minute cap\n' >&2; exit 2; }

if [ ! -f "$DATASET" ]; then
  mkdir -p "$(dirname "$DATASET")"
  awk -f "$GEN_AWK" > "$DATASET" 2> "$HERE/data/gen.log"
fi
DATASET_SHA="$(sha256sum "$DATASET" | awk '{print $1}')"
DATASET_ROWS="$(( $(awk 'END { print NR }' "$DATASET") - 1 ))"
RUN_KEY="$(printf '%s\n' "$BASE_URL" "$EFFORT" "$OVERRIDES" "$DATASET_SHA" | sha256sum | awk '{print $1}')"
RUN_ID="${RUN_ID:-run-${RUN_KEY:0:16}}"
[[ "$RUN_ID" =~ ^[A-Za-z0-9._-]+$ ]] || { printf 'RUN_ID contains unsafe path characters\n' >&2; exit 2; }
OUT="$HERE/receipts/$RUN_ID"
if [ -d "$OUT" ]; then
  rm -f "$OUT"/*.json "$OUT"/*.zip "$OUT"/*.txt "$OUT"/*.csv "$OUT"/*.sha256 "$OUT"/*.curl-error 2>/dev/null || true
fi
mkdir -p "$OUT"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/jain-e2e-studies.XXXXXX")"
cleanup() { rm -rf "$TMP_DIR"; }
trap cleanup EXIT
GENERATED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

log() { printf '  %s\n' "$*" >&2; }
hr()  { printf '%s\n' "----------------------------------------------------------------" >&2; }

# curl wrappers: write body to $1, echo HTTP status. Auth header only if AUTH_KEY set.
AUTH_ARGS=(); [ -n "$AUTH_KEY" ] && AUTH_ARGS=(-H "authorization: Bearer $AUTH_KEY")
api_get() {
  local err="$1.curl-error"
  if curl -sS --max-time 20 "${AUTH_ARGS[@]}" -o "$1" -w '%{http_code}' "$BASE_URL$2" 2>"$err"; then :; else cat "$err" >&2; return 1; fi
}
api_postj() {
  local err="$1.curl-error"
  if curl -sS --max-time 60 "${AUTH_ARGS[@]}" -o "$1" -w '%{http_code}' -H 'content-type: application/json' -X POST --data "$3" "$BASE_URL$2" 2>"$err"; then :; else cat "$err" >&2; return 1; fi
}
api_poste() {
  local err="$1.curl-error"
  if curl -sS --max-time 60 "${AUTH_ARGS[@]}" -o "$1" -w '%{http_code}' -X POST "$BASE_URL$2" 2>"$err"; then :; else cat "$err" >&2; return 1; fi
}
api_dl() {
  local err="$1.curl-error"
  if curl -sS --max-time 120 "${AUTH_ARGS[@]}" -o "$1" -w '%{http_code}' "$BASE_URL$2" 2>"$err"; then :; else cat "$err" >&2; return 1; fi
}
curl_error() {
  local file="$1"
  [ -s "$file" ] || return 0
  tr '\n' ' ' < "$file" | sed 's/[[:space:]]*$//'
}

# study result accumulators (globals; read by write_receipt)
UP_STATUS=fail; TR_STATUS=fail; CH_STATUS=fail; EX_STATUS=fail
ENGINE_OBSERVED=unknown; TERMINAL_STATE=unknown; STARFORGE_TRIALS=0; ENGINE_MATCH=true
UP_DETAIL=""; TR_DETAIL=""; CH_DETAIL=""; EX_DETAIL=""; VERDICT=fail
CHIMERA_ENABLED=false; SESSION_ID=""; MODEL_ID=""
RUNNER=unknown; EXECUTION_MODE=unknown; CLUSTER_ACCESS=false; TRAINING_ENABLED=false
HEALTH_STATUS=unknown; HEALTH_VERSION=unknown; SYSTEM_DETAIL=""
BLOCKER=""; P6_DONE=0

write_receipt() {   # assemble receipt.json + sidecar from current globals; sets VERDICT
  local verdict=pass
  for s in "$UP_STATUS" "$TR_STATUS" "$CH_STATUS" "$EX_STATUS"; do [ "$s" = pass ] || verdict=fail; done
  VERDICT="$verdict"
  local receipt_status=blocked
  [ "$verdict" = pass ] && receipt_status=complete
  [ "$verdict" = pass ] || BLOCKER="${BLOCKER:-one or more studies failed}"
  jq -n \
    --arg schema "jain.e2e-studies/v1" \
    --arg run_id "$RUN_ID" --arg mode "$MODE" --arg base "$BASE_URL" \
    --arg effort "$EFFORT" --arg runner "$RUNNER" --arg gen "$GENERATED_AT" \
    --arg ds_path "docs/release-evidence/8.0.0/e2e-studies/data/e2e.csv" \
    --arg ds_sha "$DATASET_SHA" --argjson ds_rows "${DATASET_ROWS:-0}" \
    --arg health_status "$HEALTH_STATUS" --arg health_version "$HEALTH_VERSION" \
    --arg system_detail "$SYSTEM_DETAIL" --arg receipt_status "$receipt_status" \
    --arg blocker "$BLOCKER" \
    --arg sid "$SESSION_ID" --argjson chimera "$([ "$CHIMERA_ENABLED" = true ] && echo true || echo false)" \
    --arg up "$UP_STATUS" --arg tr "$TR_STATUS" --arg ch "$CH_STATUS" --arg ex "$EX_STATUS" \
    --arg up_d "$UP_DETAIL" --arg tr_d "$TR_DETAIL" --arg ch_d "$CH_DETAIL" --arg ex_d "$EX_DETAIL" \
    --arg engine "$ENGINE_OBSERVED" --arg expect "$EXPECT_ENGINE" \
    --argjson match "$([ "${ENGINE_MATCH:-true}" = true ] && echo true || echo false)" \
    --arg terminal "$TERMINAL_STATE" --argjson starforge "${STARFORGE_TRIALS:-0}" \
    --arg model_id "$MODEL_ID" \
    --arg exc "docs/release-evidence/8.0.0/jankurai-auditor-exception.json" \
    --arg verdict "$verdict" \
    '{schema:$schema, status:$receipt_status, blocker:(if $blocker == "" then null else $blocker end),
      run_id:$run_id, generated_at:$gen, mode:$mode, base_url:$base,
      effort:$effort, runner:$runner, auditor_exception:$exc,
      preflight:{health_status:$health_status, health_version:$health_version, runtime:$system_detail},
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

fail_receipt() { BLOCKER="${1:-${BLOCKER:-early failure}}"; write_receipt; log "verdict=FAIL (early exit)"; exit 1; }

# --------------------------------------------------------------- preflight
hr; log "WQ-9 E2E studies  run_id=$RUN_ID  base=$BASE_URL  effort=$EFFORT  mode=$MODE"; hr
cp "$DATASET" "$OUT/dataset.csv"
log "dataset: rows=$DATASET_ROWS sha256=$DATASET_SHA"

if ! HEALTH_STATUS="$(api_get "$OUT/health.json" "/api/health")"; then
  fail_receipt "health request failed"
fi
if [ "$HEALTH_STATUS" != "200" ]; then
  fail_receipt "health HTTP $HEALTH_STATUS"
fi
if ! HEALTH_VERSION="$(jq -er 'select(.ok == true) | .version' "$OUT/health.json")"; then
  fail_receipt "health response missing ok=true/version"
fi
[ "$HEALTH_VERSION" = "8.0.0" ] || fail_receipt "unexpected server version $HEALTH_VERSION"

if ! SYSTEM_STATUS="$(api_get "$OUT/system.json" "/api/system")"; then
  fail_receipt "runtime metadata request failed"
fi
[ "$SYSTEM_STATUS" = "200" ] || fail_receipt "runtime metadata HTTP $SYSTEM_STATUS"
if ! RUNNER="$(jq -er '.runtime.runner' "$OUT/system.json")"; then fail_receipt "runtime.runner missing"; fi
if ! EXECUTION_MODE="$(jq -er '.runtime.execution_mode' "$OUT/system.json")"; then fail_receipt "runtime.execution_mode missing"; fi
if ! CLUSTER_ACCESS="$(jq -er '.runtime.cluster_access' "$OUT/system.json")"; then fail_receipt "runtime.cluster_access missing"; fi
if ! TRAINING_ENABLED="$(jq -er '.runtime.training_enabled' "$OUT/system.json")"; then fail_receipt "runtime.training_enabled missing"; fi
SYSTEM_DETAIL="runner=$RUNNER execution_mode=$EXECUTION_MODE cluster_access=$CLUSTER_ACCESS training_enabled=$TRAINING_ENABLED"
[ "$RUNNER" = "real" ] || fail_receipt "runtime runner is $RUNNER, expected real"
[ "$EXECUTION_MODE" = "real" ] || fail_receipt "runtime execution_mode is $EXECUTION_MODE, expected real"
[ "$CLUSTER_ACCESS" = "true" ] || fail_receipt "runtime cluster_access is $CLUSTER_ACCESS"
[ "$TRAINING_ENABLED" = "true" ] || fail_receipt "runtime training_enabled is $TRAINING_ENABLED"
log "health=$HEALTH_STATUS version=$HEALTH_VERSION $SYSTEM_DETAIL"

# =========================================================== STUDY A: upload
hr; log "STUDY (a) upload"
if ! body="$(jq -cn --arg effort "$EFFORT" --argjson overrides "$OVERRIDES" '{effort:$effort,overrides:$overrides}')"; then
  fail_receipt "invalid OVERRIDES JSON"
fi
if ! st="$(api_postj "$OUT/session-create.json" "/api/sessions" "$body")"; then
  fail_receipt "create session request failed"
fi
if [ "$st" != "200" ]; then UP_DETAIL="create HTTP $st"; log "create session -> $st"; fail_receipt "create session HTTP $st"; fi
if ! SESSION_ID="$(jq -er '.id | strings' "$OUT/session-create.json")"; then fail_receipt "session response missing id"; fi
if ! CHIMERA_ENABLED="$(jq -er '.resolved_knobs.chimera_enabled' "$OUT/session-create.json")"; then fail_receipt "session response missing chimera_enabled"; fi
if ! jq '.resolved_knobs' "$OUT/session-create.json" > "$OUT/resolved_knobs.json"; then
  fail_receipt "session response has invalid resolved_knobs"
fi
log "session=$SESSION_ID chimera_enabled=$CHIMERA_ENABLED"
[ "$CHIMERA_ENABLED" = "true" ] || { CH_STATUS=fail; CH_DETAIL="resolved_knobs.chimera_enabled=$CHIMERA_ENABLED"; fail_receipt "Chimera is not enabled"; }

# Upload: role=train part FIRST, then the file (filename MUST end .csv). File part name is not
# validated by the server, but WQ-9 uses "file".
if ! st="$(curl -sS --max-time 120 "${AUTH_ARGS[@]}" -o "$OUT/upload.json" -w '%{http_code}' \
      -F 'role=train' \
      -F "file=@$DATASET;type=text/csv;filename=e2e.csv" \
      -X POST "$BASE_URL/api/sessions/$SESSION_ID/files" 2>"$OUT/upload.curl-error")"; then
  detail="$(curl_error "$OUT/upload.curl-error")"
  fail_receipt "dataset upload request failed${detail:+: $detail}"
fi
if ! UPLOAD_FILE_STATUS="$(jq -er '.files[0].status // "none"' "$OUT/upload.json")"; then UPLOAD_FILE_STATUS=invalid; fi
if ! POST_UPLOAD_STATE="$(jq -er '.state // "unknown"' "$OUT/upload.json")"; then POST_UPLOAD_STATE=invalid; fi
log "upload HTTP $st  files[0].status=$UPLOAD_FILE_STATUS  state=$POST_UPLOAD_STATE"

if ! events_status="$(api_get "$OUT/events-upload.json" "/api/sessions/$SESSION_ID/events?after=0")"; then
  fail_receipt "upload events request failed"
fi
[ "$events_status" = "200" ] || fail_receipt "upload events HTTP $events_status"
if ! HAS_UP_START="$(jq -er '[.[]|select(.kind=="upload.started")]|length' "$OUT/events-upload.json")"; then HAS_UP_START=0; fi
if ! HAS_UP_DONE="$(jq -er '[.[]|select(.kind=="upload.complete")]|length' "$OUT/events-upload.json")"; then HAS_UP_DONE=0; fi
if ! TARGET_REQUIRED="$(jq -er '[.[]|select(.kind=="target.required")]|length' "$OUT/events-upload.json")"; then TARGET_REQUIRED=0; fi
if [ "$st" = "200" ] && [ "$UPLOAD_FILE_STATUS" = "uploaded" ] && [ "${HAS_UP_START:-0}" -ge 1 ] && [ "${HAS_UP_DONE:-0}" -ge 1 ]; then
  UP_STATUS=pass
fi
UP_DETAIL="http=$st file_status=$UPLOAD_FILE_STATUS upload.started=$HAS_UP_START upload.complete=$HAS_UP_DONE target.required=$TARGET_REQUIRED"
log "STUDY (a) upload => $UP_STATUS  [$UP_DETAIL]"

# ================================================= drive training to terminal
hr; log "driving training (auto-start aware)"
get_state() {
  local state_status
  state_status="$(api_get "$OUT/state.json" "/api/sessions/$SESSION_ID")"
  [ "$state_status" = "200" ] || return 1
  jq -er '.state | strings' "$OUT/state.json"
}
started_kick=false
target_kick=false
for _ in $(seq 1 20); do
  if ! cs="$(get_state)"; then
    TERMINAL_STATE=unknown
    BLOCKER="session state request failed while starting training"
    break
  fi
  if [ "$TARGET_REQUIRED" -ge 1 ] && [ "$target_kick" = false ]; then
    log "target.required observed -> approving target 'target'"
    if ! st="$(api_postj "$OUT/target.json" "/api/sessions/$SESSION_ID/target" '{"target":"target","approved":true}')"; then
      BLOCKER="target approval request failed"
      break
    fi
    case "$st" in 200|409) ;; *) BLOCKER="target approval HTTP $st"; break ;; esac
    target_kick=true
  fi
  case "$cs" in
    awaiting_target)
      if [ "$target_kick" = false ]; then
        log "state=awaiting_target -> approving target 'target'"
        if ! st="$(api_postj "$OUT/target.json" "/api/sessions/$SESSION_ID/target" '{"target":"target","approved":true}')"; then
          BLOCKER="target approval request failed"
          break
        fi
        case "$st" in 200|409) ;; *) BLOCKER="target approval HTTP $st"; break ;; esac
        target_kick=true
      fi ;;
    ready_to_train)
      if [ "$started_kick" = false ]; then
        log "state=ready_to_train -> POST /start"
        if ! st="$(api_poste "$OUT/start.json" "/api/sessions/$SESSION_ID/start")"; then
          BLOCKER="training start request failed"
          break
        fi
        case "$st" in 200|409) ;; *) BLOCKER="training start HTTP $st"; break ;; esac
        started_kick=true
      fi ;;
    training|complete|prediction_ready|failed|cancelled)
      log "state=$cs -> training underway/terminal"; break ;;
    *) : ;;   # created/uploading/uploaded/profiling: settle, retry
  esac
  sleep 2
done

deadline=$(( $(date +%s) + POLL_TIMEOUT ))
while :; do
  if ! TERMINAL_STATE="$(get_state)"; then
    TERMINAL_STATE=unknown
    BLOCKER="session state request failed while polling training"
    break
  fi
  case "$TERMINAL_STATE" in
    complete|prediction_ready) log "training terminal: $TERMINAL_STATE"; break ;;
    failed|cancelled)          log "training terminal FAIL: $TERMINAL_STATE"; break ;;
  esac
  [ "$(date +%s)" -ge "$deadline" ] && { BLOCKER="training poll timeout after ${POLL_TIMEOUT}s (state=$TERMINAL_STATE)"; log "$BLOCKER"; break; }
  sleep "$POLL_INTERVAL"
done

# ===================================================== STUDY B: training/engine
hr; log "STUDY (b) training + engine observation"
if ! events_status="$(api_get "$OUT/events.json" "/api/sessions/$SESSION_ID/events?after=0")"; then
  detail="$(curl_error "$OUT/events.json.curl-error")"
  fail_receipt "training events request failed${detail:+: $detail}"
fi
[ "$events_status" = "200" ] || fail_receipt "training events HTTP $events_status"
if ! P6_DONE="$(jq -er '[.[]|select(.kind=="training.progress" and (.payload.phase==6) and (.payload.event=="done"))]|length' "$OUT/events.json")"; then P6_DONE=0; fi
if ! NOTE6="$(jq -er '[.[]|select(.kind=="training.progress" and (.payload.phase==6))|.payload.note // empty]|join(" | ")' "$OUT/events.json")"; then NOTE6=""; fi
if   printf '%s' "$NOTE6" | grep -qi 'Lime';  then ENGINE_OBSERVED=lime
elif printf '%s' "$NOTE6" | grep -qi 'Prime'; then ENGINE_OBSERVED=prime
else ENGINE_OBSERVED=legacy; fi
FAILURE_DETAIL="$(jq -r '[.[] | select((.kind // "" | test("failed|error";"i")) or (.level // "" | ascii_downcase == "error")) | (.message // .payload.error // .payload.reason // empty)] | unique | join(" | ")' "$OUT/events.json")"
case "$TERMINAL_STATE" in complete|prediction_ready)
  [ "$P6_DONE" -ge 1 ] && TR_STATUS=pass || TR_STATUS=fail ;;
*) TR_STATUS=fail ;;
esac
ENGINE_MATCH=true
if [ "$EXPECT_ENGINE" != "any" ] && [ "$ENGINE_OBSERVED" != "$EXPECT_ENGINE" ]; then ENGINE_MATCH=false; fi
if [ "$ENGINE_STRICT" = "1" ] && [ "$ENGINE_MATCH" = false ]; then TR_STATUS=fail; fi
TR_DETAIL="terminal=$TERMINAL_STATE phase6_done=$P6_DONE engine=$ENGINE_OBSERVED expect=$EXPECT_ENGINE match=$ENGINE_MATCH strict=$ENGINE_STRICT"
[ -n "$FAILURE_DETAIL" ] && TR_DETAIL="$TR_DETAIL failure=$FAILURE_DETAIL"
[ "$TR_STATUS" = pass ] || BLOCKER="${BLOCKER:-${FAILURE_DETAIL:-training did not produce required terminal/phase-6 evidence}}"
log "STUDY (b) training => $TR_STATUS  [$TR_DETAIL]"

# ===================================================== STUDY C: chimera/starforge
hr; log "STUDY (c) chimera classification (starforge trial)"
if [ "$TR_STATUS" = pass ]; then
  if ! session_status="$(api_get "$OUT/session-final.json" "/api/sessions/$SESSION_ID")"; then
    CH_DETAIL="session readback request failed"
  elif [ "$session_status" != "200" ]; then
    CH_DETAIL="session readback HTTP $session_status"
  fi
  MODEL_ID=""
  if [ "${session_status:-}" = "200" ]; then
    MODEL_ID="$(jq -r '(.artifacts // []) | map(select(.kind=="model")) | .[0].id // empty' "$OUT/session-final.json")"
  fi
  [ -z "$MODEL_ID" ] && MODEL_ID="$(jq -r '[.[]|select(.kind=="artifact.ready" and (.payload.kind=="model"))][0].payload.artifact_id // empty' "$OUT/events.json")"
  log "model_id=$MODEL_ID"
  if [ -n "$MODEL_ID" ]; then
    if ! st="$(api_dl "$OUT/model.zip" "/api/artifacts/$MODEL_ID/download")"; then
      CH_DETAIL="model download request failed"
    elif [ "$st" = "200" ] && mkdir -p "$TMP_DIR/model" && unzip -q "$OUT/model.zip" -d "$TMP_DIR/model"; then
      if [ ! -s "$TMP_DIR/model/manifest.json" ]; then
        CH_DETAIL="dl=$st manifest.json missing"
      elif ! jq empty "$TMP_DIR/model/manifest.json"; then
        CH_DETAIL="dl=$st manifest.json invalid"
      else
        cp "$TMP_DIR/model/manifest.json" "$OUT/model-manifest.json"
        STARFORGE_TRIALS="$(jq -r '[.run.model_trials[]? | select(.backend=="starforge" and .status=="completed")]|length' "$OUT/model-manifest.json")"
        STARFORGE_COMPONENTS="$(jq -r '(.run.frontier_components // []) | length' "$OUT/model-manifest.json")"
        MISSING_WEIGHTS="$(jq -r '[.run.frontier_components[]? | select(.weights_present != true)] | length' "$OUT/model-manifest.json")"
        STARFORGE_TRIALS="${STARFORGE_TRIALS:-0}"
        STARFORGE_COMPONENTS="${STARFORGE_COMPONENTS:-0}"
        MISSING_WEIGHTS="${MISSING_WEIGHTS:-0}"
        if [ "$CHIMERA_ENABLED" = "true" ] && [ "$STARFORGE_TRIALS" -ge 1 ] && [ "$STARFORGE_COMPONENTS" -ge 1 ] && [ "$MISSING_WEIGHTS" -eq 0 ]; then
          CH_STATUS=pass
        else
          BLOCKER="${BLOCKER:-model manifest lacks completed Starforge trial or reports missing weights}"
        fi
        CH_DETAIL="dl=$st manifest=present chimera_enabled=$CHIMERA_ENABLED starforge_completed_trials=$STARFORGE_TRIALS frontier_components=$STARFORGE_COMPONENTS missing_weights=$MISSING_WEIGHTS"
      fi
    else
      CH_DETAIL="model download HTTP $st or archive extraction failed"
    fi
  else CH_DETAIL="no model artifact id found"; fi
else CH_DETAIL="skipped (training did not complete)"; fi
log "STUDY (c) chimera => $CH_STATUS  [$CH_DETAIL]"

# ===================================================== STUDY D: algorithm export
hr; log "STUDY (d) algorithm export (leak-free)"
if [ "$TR_STATUS" = pass ]; then
  if ! st="$(api_dl "$OUT/export.zip" "/api/sessions/$SESSION_ID/export/algorithm")"; then
    EX_DETAIL="export download request failed"
  elif [ "$st" = "200" ] && unzip -Z1 "$OUT/export.zip" > "$OUT/export-entries.txt"; then
    HAS_README="$(grep -c '^README\.md$' "$OUT/export-entries.txt" || true)"
    HAS_MANIFEST="$(grep -c '^MANIFEST\.txt$' "$OUT/export-entries.txt" || true)"
    HAS_PY="$(grep -cE '^invention(-[0-9]+)?/[^/]+/model\.py$' "$OUT/export-entries.txt" || true)"
    HAS_RS="$(grep -cE '^invention(-[0-9]+)?/[^/]+/model\.rs$' "$OUT/export-entries.txt" || true)"
    HAS_DSL="$(grep -cE '^invention(-[0-9]+)?/[^/]+/genome\.dsl$' "$OUT/export-entries.txt" || true)"
    RESTRICTED_HITS="$(grep -icE "$RESTRICTED_RE" "$OUT/export-entries.txt" || true)"
    INVALID_ENTRIES=0
    while IFS= read -r entry; do
      if [[ "$entry" != "README.md" && "$entry" != "MANIFEST.txt" && ! "$entry" =~ ^MANIFEST-[0-9]+\.txt$ && ! "$entry" =~ ^invention(-[0-9]+)?/(MANIFEST\.txt|[^/]+/(model\.py|model\.rs|genome\.dsl))$ ]]; then
        INVALID_ENTRIES=$((INVALID_ENTRIES + 1))
      fi
    done < "$OUT/export-entries.txt"
    if [ "$HAS_README" -ge 1 ] && [ "$HAS_MANIFEST" -ge 1 ] && [ "$HAS_PY" -ge 1 ] && [ "$HAS_RS" -ge 1 ] && [ "$HAS_DSL" -ge 1 ] && [ "$RESTRICTED_HITS" -eq 0 ] && [ "$INVALID_ENTRIES" -eq 0 ]; then
      EX_STATUS=pass
    else
      BLOCKER="${BLOCKER:-algorithm export failed the approved allowlist}"
    fi
    EX_DETAIL="dl=$st readme=$HAS_README manifest=$HAS_MANIFEST py=$HAS_PY rs=$HAS_RS dsl=$HAS_DSL restricted_name_hits=$RESTRICTED_HITS invalid_entries=$INVALID_ENTRIES"
  else EX_DETAIL="export download HTTP $st or archive listing failed"; fi
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
