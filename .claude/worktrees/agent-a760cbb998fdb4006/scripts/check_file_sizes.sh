#!/usr/bin/env bash
set -euo pipefail

max_loc=2000
warn_loc=1500
status=0

while IFS= read -r file; do
  case "$file" in
    agent/repo-score.json|agent/repo-score.md)
      continue
      ;;
    agent/score-history.csv|agent/score-history.jsonl)
      continue
      ;;
    docs/archive/*|paper/figs/*.eps|target/*)
      continue
      ;;
    benchmark-results/*)
      continue
      ;;
    agent/baselines/*)
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
