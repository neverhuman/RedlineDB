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
# Required: pinned `jankurai` release binary installed by ops/ci/lib.sh.

set -euo pipefail

BASE_REF="${BASE_REF:-origin/main}"
LOG_DIR="${LOG_DIR:-.jankurai/staged-gate}"
mkdir -p "$LOG_DIR"

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"
ci_install_jankurai_logged "$LOG_DIR/install.log"

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

generated_zone_paths=()
if [ -f ".jankurai/generated-zones.toml" ]; then
  while IFS= read -r zone_path; do
    [ -z "$zone_path" ] && continue
    generated_zone_paths+=("$zone_path")
  done < <(sed -n 's/^[[:space:]]*path[[:space:]]*=[[:space:]]*"\([^"]*\)".*$/\1/p' .jankurai/generated-zones.toml)
fi

is_generated_zone_path() {
  local candidate="$1"

  if [ "$candidate" = ".jankurai/generated-zones.toml" ]; then
    return 1
  fi

  local zone_path
  for zone_path in "${generated_zone_paths[@]}"; do
    [ -z "$zone_path" ] && continue
    if [[ "$zone_path" == */ ]]; then
      case "$candidate" in
        "$zone_path"*) return 0 ;;
      esac
    elif [ "$candidate" = "$zone_path" ]; then
      return 0
    fi
  done

  return 1
}

is_same_pr_script_missing_only() {
  local json_out="$1"
  local script_path

  if ! grep -q '"summary":[[:space:]]*"blocked: 1 new hard, 0 worsened, 0 always-block finding(s)"' "$json_out"; then
    return 1
  fi
  if ! grep -q '"matched_term":[[:space:]]*"ci.local-parity.script-missing"' "$json_out"; then
    return 1
  fi

  script_path="$(
    sed -n 's/.*workflow references missing script `\([^`]*\)`.*/\1/p' "$json_out" |
      head -n 1
  )"
  if [ -z "$script_path" ]; then
    return 1
  fi

  git cat-file -e "HEAD:$script_path" 2>/dev/null
}

failed=0
blocked_files=()
skipped_generated=0
skipped_same_pr_scripts=0
i=0
for path in "${changed_files[@]}"; do
  [ -z "$path" ] && continue
  i=$((i + 1))

  case "$path" in
    target/*|node_modules/*|*.lock) continue ;;
  esac

  if is_generated_zone_path "$path"; then
    skipped_generated=$((skipped_generated + 1))
    continue
  fi

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
    if [ -s "$json_out" ] && is_same_pr_script_missing_only "$json_out"; then
      skipped_same_pr_scripts=$((skipped_same_pr_scripts + 1))
      printf 'jankurai staged-gate: %s references script added in this PR; continuing\n' \
        "$path"
      continue
    fi
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

printf 'jankurai staged-gate: %d file(s) clean vs %s (%d generated-zone file(s) skipped)\n' \
  "${#changed_files[@]}" "$BASE_REF" "$skipped_generated"
if [ "$skipped_same_pr_scripts" -ne 0 ]; then
  printf 'jankurai staged-gate: %d same-PR workflow script reference(s) verified in HEAD\n' \
    "$skipped_same_pr_scripts"
fi
exit 0
