#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' \
  'nightly-fuzz is disabled: SQLite parity coverage, benchmark, report, sentinel, and proof evidence must be produced only through the pinned neverhuman/redline-testing release artifact.' \
  >&2
exit 1
