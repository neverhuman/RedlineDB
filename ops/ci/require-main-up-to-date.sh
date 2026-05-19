#!/usr/bin/env bash
# Shared branch freshness gate.
#
# Fails when the current branch does not contain the latest fetched
# `origin/main`. This is used by the managed pre-commit hook and by CI so
# local commits and PR checks enforce the same branch-up-to-date contract.
#
# Environment overrides:
#   REQUIRE_MAIN_REMOTE  - remote name to inspect (default: origin)
#   REQUIRE_MAIN_BRANCH  - branch name to require (default: main)

set -euo pipefail

REMOTE="${REQUIRE_MAIN_REMOTE:-origin}"
MAIN_BRANCH="${REQUIRE_MAIN_BRANCH:-main}"

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$repo_root"

if ! git rev-parse --verify HEAD >/dev/null 2>&1; then
  exit 0
fi

current_branch="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || true)"
if [ -z "$current_branch" ] || [ "$current_branch" = "HEAD" ]; then
  printf 'branch freshness gate: detached HEAD cannot be validated\n' >&2
  exit 1
fi

if [ "$current_branch" = "$MAIN_BRANCH" ]; then
  exit 0
fi

printf "Checking whether '%s' contains the latest %s/%s...\n" \
  "$current_branch" "$REMOTE" "$MAIN_BRANCH" >&2

if ! git fetch "$REMOTE" "$MAIN_BRANCH" --quiet; then
  printf 'branch freshness gate: failed to fetch %s/%s\n' "$REMOTE" "$MAIN_BRANCH" >&2
  exit 1
fi

main_ref="$REMOTE/$MAIN_BRANCH"
if ! git rev-parse --verify "$main_ref" >/dev/null 2>&1; then
  printf 'branch freshness gate: %s is not available locally after fetch\n' "$main_ref" >&2
  exit 1
fi

if ! git merge-base --is-ancestor "$main_ref" HEAD; then
  printf '\n❌ Commit blocked.\n' >&2
  printf 'Your branch does not contain the latest %s.\n\n' "$main_ref" >&2
  printf 'Run:\n' >&2
  printf '  git fetch %s\n' "$REMOTE" >&2
  printf '  git rebase %s\n\n' "$main_ref" >&2
  printf 'Then try committing again.\n' >&2
  exit 1
fi

exit 0
