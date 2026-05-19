#!/usr/bin/env bash
# Per-file save-gate audit for PR-changed files.
#
# Mirrors the local pre-commit hook's semantics (see
# tools/jankurai-hooks/pre-commit): every changed file is fed to
# `jankurai audit-file --mode save-gate` with a baseline drawn from the
# merge-base. Any blocked file fails the lane.
#
# Usage:
#   bash ops/ci/jankurai-staged-gate.sh                # uses origin/main
#   BASE_REF=origin/main bash ops/ci/jankurai-staged-gate.sh
#
# Required: `jankurai` 1.4.3+ on PATH.

set -euo pipefail

BASE_REF="${BASE_REF:-origin/main}"
LOG_DIR="${LOG_DIR:-target/jankurai/staged-gate}"
mkdir -p "$LOG_DIR"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

bash "$ROOT/ops/ci/require-main-up-to-date.sh"

if ! command -v jankurai >/dev/null 2>&1; then
  echo "jankurai not on PATH; skipping staged-gate (soft gate)" >&2
  exit 0
fi

# Resolve the merge base so we diff against the branch divergence point,
# not a moving target on the base branch.
if ! git rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
  echo "BASE_REF=$BASE_REF not resolvable; skipping staged-gate" >&2
  exit 0
fi
base_sha="$(git merge-base "$BASE_REF" HEAD)"
if [ -z "$base_sha" ]; then
  echo "no merge base with $BASE_REF; skipping staged-gate" >&2
  exit 0
fi

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/jankurai-staged-gate.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT

changed_files=()
while IFS= read -r -d '' entry; do
  changed_files+=("$entry")
done < <(git diff --name-only --diff-filter=ACMR -z "$base_sha"...HEAD)

if [ "${#changed_files[@]}" -eq 0 ]; then
  echo "no changed files vs $BASE_REF; nothing to gate"
  exit 0
fi

failed=0
blocked_files=()
i=0
for path in "${changed_files[@]}"; do
  [ -z "$path" ] && continue
  i=$((i + 1))

  case "$path" in
    target/*|node_modules/*|*.lock) continue ;;
  esac

  unset baseline_arg
  baseline_arg=()
  baseline_file="$tmp_dir/baseline-$i"
  op="modify"
  if git cat-file -e "$base_sha:$path" 2>/dev/null && git show "$base_sha:$path" > "$baseline_file" 2>/dev/null; then
    baseline_arg=(--baseline "$baseline_file")
  else
    op="create"
    : > "$baseline_file"
    baseline_arg=(--baseline "$baseline_file")
  fi

  json_out="$tmp_dir/result-$i.json"

  set +e
  git show "HEAD:$path" 2>/dev/null \
    | jankurai audit-file \
        --path "$path" \
        --candidate - \
        --op "$op" \
        --mode save-gate \
        --format json \
        --json-out "$json_out" \
        ${baseline_arg[@]+"${baseline_arg[@]}"} \
        > "$tmp_dir/stdout-$i" 2> "$tmp_dir/stderr-$i"
  rc=$?
  set -e

  if [ "$rc" -ne 0 ]; then
    failed=1
    blocked_files+=("$path")
    cp "$json_out" "$LOG_DIR/blocked-$i.json" 2>/dev/null || true
    summary=""
    if [ -s "$json_out" ]; then
      summary="$(sed -n 's/^[[:space:]]*"summary":[[:space:]]*"\(.*\)",*$/\1/p' "$json_out" | head -n 1)"
    fi
    printf '::error file=%s::%s\n' "$path" "${summary:-blocked by jankurai save-gate}"
  fi
done

if [ "$failed" -ne 0 ]; then
  printf 'jankurai staged-gate blocked %d file(s) vs %s (merge-base %s)\n' \
    "${#blocked_files[@]}" "$BASE_REF" "$base_sha" >&2
  for p in "${blocked_files[@]}"; do
    printf '  %s\n' "$p" >&2
  done
  exit 1
fi

printf 'jankurai staged-gate: %d file(s) clean vs %s\n' "${#changed_files[@]}" "$BASE_REF"
exit 0
