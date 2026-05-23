#!/usr/bin/env bash
set -euo pipefail

max_loc=2000
warn_loc=1500
status=0

while IFS= read -r file; do
  case "$file" in
    Cargo.lock)
      continue
      ;;
    .jankurai/repo-score.json|.jankurai/repo-score.md)
      continue
      ;;
    .jankurai/score-history.csv|.jankurai/score-history.jsonl)
      continue
      ;;
    docs/archive/*|paper/figs/*.eps|target/*)
      continue
      ;;
    tips/feedback/*|tips/performance/*)
      continue
      ;;
    benchmark-results/*)
      continue
      ;;
    .jankurai/baselines/*)
      continue
      ;;
    docs/architecture/ENGINEERING_SPEC.md)
      continue
      ;;
  esac
  [[ -f "$file" ]] || continue
  if ! grep -Iq . "$file"; then
    continue
  fi
  lines=$(wc -l < "$file")
  if [[ "$lines" -gt "$max_loc" ]]; then
    printf '%6d %s\n' "$lines" "$file"
    status=1
  elif [[ "$lines" -gt "$warn_loc" ]]; then
    printf 'warning: %6d %s\n' "$lines" "$file" >&2
  fi
done < <(git ls-files -co --exclude-standard)

exit "$status"
