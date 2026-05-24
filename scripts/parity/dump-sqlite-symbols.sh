#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' \
  'dump-sqlite-symbols is disabled: RedlineDB must not generate local SQLite parity proof artifacts under target/proof. The FFI symbol-diff test computes its reference in memory; official SQLite parity evidence must come only from the pinned neverhuman/redline-testing release artifact.' \
  >&2
exit 1
