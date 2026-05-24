#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' \
  'write-sqlite-full-parity-receipts is disabled: SQLite parity coverage, benchmark, report, sentinel, and proof evidence must be produced only through the pinned neverhuman/redline-testing release artifact. Use `rtk just redline-testing-official` or `rtk just sqlite-parity-report-update`.' \
  >&2
exit 1
