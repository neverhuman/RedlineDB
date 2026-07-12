#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' \
  'closeout-prs.sh is retired for the v8 release.' \
  'It must not create commits, rewrite review branches, or merge a family in bulk.' \
  'Use the receipt-producing splitctl PR, worktree, and immutable-tag commands.' >&2
exit 2
