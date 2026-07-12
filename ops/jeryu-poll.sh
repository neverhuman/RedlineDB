#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' \
  'Autonomous PR merge polling is disabled for the v8 release.' \
  'Run the receipt-producing splitctl lifecycle commands explicitly after CI and review.' >&2
exit 2
