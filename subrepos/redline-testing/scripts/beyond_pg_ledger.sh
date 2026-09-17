#!/usr/bin/env bash
# beyond_pg_ledger.sh — generate a categorized failure ledger for the
# beyond-SQLite Postgres-oracle target-compare lane.
#
# Input:  $1 = path to beyond_sqlite.raw.jsonl emitted by `redline-testing
#              run --suite beyond_sqlite --target-bin <redlinedb>`
# Output: $2 = path to write the markdown ledger
#         $3 = (optional) path to write a JSONL stream of failing records
#
# The ledger summarizes the `beyond_sqlite_target` profile (psql ↔ redlinedb)
# rolling failures up by `category` and by stderr root-cause buckets so a
# follow-up agent can split the work into parallel tracks.

set -euo pipefail

if [[ $# -lt 2 ]]; then
    echo "usage: $0 <raw.jsonl> <ledger.md> [failures.jsonl]" >&2
    exit 2
fi

raw="$1"
ledger="$2"
failures="${3:-}"

if [[ ! -f "$raw" ]]; then
    echo "input not found: $raw" >&2
    exit 2
fi

mkdir -p "$(dirname "$ledger")"
if [[ -n "$failures" ]]; then
    mkdir -p "$(dirname "$failures")"
fi

# Pull the target-compare records out and write the failure stream.
target_total=$(jq -s '[.[] | select(.profile == "beyond_sqlite_target")] | length' "$raw")
target_passed=$(jq -s '[.[] | select(.profile == "beyond_sqlite_target" and .status == "passed")] | length' "$raw")
target_failed=$(jq -s '[.[] | select(.profile == "beyond_sqlite_target" and .status == "failed")] | length' "$raw")
target_skipped=$(jq -s '[.[] | select(.profile == "beyond_sqlite_target" and .status == "skipped")] | length' "$raw")

oracle_total=$(jq -s '[.[] | select(.profile == "beyond_sqlite_oracle")] | length' "$raw")
oracle_passed=$(jq -s '[.[] | select(.profile == "beyond_sqlite_oracle" and .status == "passed")] | length' "$raw")
oracle_skipped=$(jq -s '[.[] | select(.profile == "beyond_sqlite_oracle" and .status == "skipped")] | length' "$raw")

if [[ -n "$failures" ]]; then
    jq -c 'select(.profile == "beyond_sqlite_target" and .status == "failed")' "$raw" > "$failures"
fi

# Failure shape: ref_exit vs target_exit.
ref0_tgt0=$(jq -s '[.[] | select(.profile == "beyond_sqlite_target" and .status == "failed" and .reference_exit_code == 0 and .target_exit_code == 0)] | length' "$raw")
ref0_tgt1=$(jq -s '[.[] | select(.profile == "beyond_sqlite_target" and .status == "failed" and .reference_exit_code == 0 and .target_exit_code != null and .target_exit_code != 0)] | length' "$raw")
ref0_tgtnull=$(jq -s '[.[] | select(.profile == "beyond_sqlite_target" and .status == "failed" and .reference_exit_code == 0 and .target_exit_code == null)] | length' "$raw")

# Category breakdown.
category_rows=$(jq -sr '
  [.[] | select(.profile == "beyond_sqlite_target")]
  | group_by(.category)
  | map({
      category: .[0].category,
      total: length,
      passed: ([.[] | select(.status == "passed")] | length),
      failed: ([.[] | select(.status == "failed")] | length),
      skipped: ([.[] | select(.status == "skipped")] | length)
    })
  | sort_by(-.failed, -.total)
  | .[]
  | "| \(.category) | \(.total) | \(.passed) | \(.failed) | \(.skipped) |"
' "$raw")

# Root-cause buckets driven by target_stderr_head substrings.
bucket() {
    local label="$1"
    local pattern="$2"
    local count
    count=$(jq -s --arg pattern "$pattern" '
      [.[] | select(.profile == "beyond_sqlite_target" and .status == "failed")]
      | map(select((.target_stderr_head // "") | contains($pattern)))
      | length
    ' "$raw")
    if [[ "$count" -gt 0 ]]; then
        echo "| $label | \`$pattern\` | $count |"
    fi
}

# Order buckets from highest expected count down so the table reads as a
# triage queue.
stderr_table=$(
    bucket "Unsupported SQL function" "unsupported function"
    bucket "Unsupported SQL expression" "unsupported expression"
    bucket "Generic unsupported SQL" "unsupported sql"
    bucket "Parser: unexpected token" "Expected:"
    bucket "Datatype mismatch" "datatype mismatch"
    bucket "Unknown column/table" "no such"
    bucket "Constraint violation" "constraint"
    bucket "Transaction error" "transaction"
)

stderr_quiet=$(jq -s '
  [.[] | select(.profile == "beyond_sqlite_target" and .status == "failed")]
  | map(select((.target_stderr_head // "") == ""))
  | length
' "$raw")

# Top 20 highest-confidence engine errors (first-line stderr counted across
# cases) so the follow-up agent has a triage queue ordered by impact.
stderr_top=$(jq -sr '
  [.[] | select(.profile == "beyond_sqlite_target" and .status == "failed")]
  | map(.target_stderr_head // "")
  | map(select(. != ""))
  | group_by(.)
  | map({err: .[0], count: length})
  | sort_by(-.count)
  | .[0:20]
  | .[]
  | "| `\(.err)` | \(.count) |"
' "$raw")

# Sample 5 categories with engine-side example for the follow-up to inspect.
sample_section=""
top_cats=$(jq -sr '
  [.[] | select(.profile == "beyond_sqlite_target" and .status == "failed")]
  | group_by(.category)
  | map({c: .[0].category, n: length})
  | sort_by(-.n) | .[0:5] | .[] | .c
' "$raw")

while IFS= read -r cat; do
    [[ -z "$cat" ]] && continue
    examples=$(jq -sr --arg cat "$cat" '
      [.[] | select(.profile == "beyond_sqlite_target" and .status == "failed" and .category == $cat)]
      | .[0:3]
      | .[]
      | . as $row
      | (($row.target_stderr_head // "") | if . == "" then ($row.diagnostic // "") else . end) as $err
      | (if ($err | length) > 220 then ($err[0:220] + "...") else $err end) as $shortErr
      | "- **\($row.case_id)** `\($row.name)` -- \($shortErr)"
    ' "$raw")
    sample_section+=$'\n'"### $cat"$'\n\n'"$examples"$'\n'
done <<< "$top_cats"

ts=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
git_rev="unknown"
if git -C "$(dirname "$0")/.." rev-parse HEAD >/dev/null 2>&1; then
    git_rev=$(git -C "$(dirname "$0")/.." rev-parse --short HEAD)
fi

{
    printf '# Beyond-SQLite Postgres gap-closure ledger\n\n'
    printf 'Generated: %s (redline-testing rev `%s`)\n\n' "$ts" "$git_rev"
    printf 'Source: `%s`\n\n' "$raw"
    printf '## Headline\n\n'
    printf -- '- **%s** beyond-SQLite cases in corpus (`corpus/beyond_sqlite/generated_manifest.json`)\n' "$oracle_total"
    printf -- '- **%s** pass the psql self-compare ship gate\n' "$oracle_passed"
    printf -- '- **%s** skip (postgres unavailable or reference-side nonzero exit)\n' "$oracle_skipped"
    printf -- '- **%s** target-compare attempts (psql vs `redlinedb`)\n' "$target_total"
    printf -- '- **%s** target-compare passed\n' "$target_passed"
    printf -- '- **%s** target-compare failed\n' "$target_failed"
    printf -- '- **%s** target-compare skipped\n\n' "$target_skipped"
    printf '## Failure shape\n\n'
    printf '| Pattern | Count | Meaning |\n|---|---|---|\n'
    printf '| ref=0, tgt=0 | %s | both exit clean, stdout differs (semantic / formatting gap) |\n' "$ref0_tgt0"
    printf '| ref=0, tgt!=0 | %s | psql ok, redlinedb errored (missing feature) |\n' "$ref0_tgt1"
    printf '| ref=0, tgt=null | %s | psql ok, redlinedb invocation failed (spawn/timeout) |\n\n' "$ref0_tgtnull"
    printf '## Category breakdown\n\n'
    printf '| Category | Total | Passed | Failed | Skipped |\n|---|---|---|---|---|\n'
    printf '%s\n' "$category_rows"
    printf '\n## Root-cause buckets (target stderr head)\n\n'
    printf '| Bucket | Substring | Failed cases |\n|---|---|---|\n'
    printf '%s\n' "$stderr_table"
    printf '\nFailures with empty stderr (stdout-diff only): **%s**.\n\n' "$stderr_quiet"
    printf '## Top 20 distinct engine errors\n\n'
    printf '| Stderr head | Count |\n|---|---|\n'
    printf '%s\n\n' "$stderr_top"
    printf '## Sample failures per top category\n'
    printf '%s\n\n' "$sample_section"
    printf '## Suggested follow-up tracks\n\n'
    printf 'Each category above is a natural parallel work track. Highest leverage:\n\n'
    printf '1. **BEYOND_RICH_TYPES** -- booleans, decimal, uuid, timestamptz; mostly stderr-clean stdout-diff (`t/f` vs `1/0`, decimal precision). Could likely be closed with extended normalizers AND/OR redlinedb adding pg-style boolean rendering.\n'
    printf '2. **BEYOND_JSONB_INDEXING** -- jsonb operators (`@>`, `->`, `->>`); need parser + executor work in `crates/sql/`.\n'
    printf '3. **BEYOND_COLLATIONS_ILIKE** -- ILIKE, collation-aware ORDER BY; need ICU collation hookup.\n'
    printf '4. **BEYOND_PORTABILITY_SYNTAX** -- DEFAULT in VALUES, standalone VALUES, FILTER clause, MERGE/ON CONFLICT variants; parser-level work.\n'
    printf '5. **BEYOND_STORED_PROCEDURES / MATERIALIZED_VIEWS / LISTEN_NOTIFY** -- entire feature surfaces not present in redlinedb; either gate out or full implementation.\n\n'
    printf 'See `%s` for per-case diagnostics. Failure-only stream available at `%s` for filter-driven re-runs.\n' "$raw" "$failures"
} > "$ledger"

echo "ledger=$ledger"
echo "failures=$failures"
echo "target_total=$target_total target_passed=$target_passed target_failed=$target_failed target_skipped=$target_skipped"
