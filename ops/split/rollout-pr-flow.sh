#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' \
  'rollout-pr-flow.sh is retired for the v8 release.' \
  'Use splitctl jeryu-local PR lifecycle commands one repository at a time;' \
  'each command requires an explicit receipt and protection readback.' >&2
exit 2
