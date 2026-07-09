#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

log 'fast lane: shell syntax'
mapfile -t sh_files < <(find ops scripts -type f -name '*.sh' | sort)
for f in "${sh_files[@]}"; do bash -n "$f"; done

log 'fast lane: python compile'
mapfile -t py_files < <(find ops -type f -name '*.py' | sort)
python3 -m py_compile "${py_files[@]}"

log 'fast lane: deterministic pytest materializer tests'
if python3 -c 'import pytest' 2>/dev/null; then
  python3 -m pytest -q ops/split/tests
else
  for t in ops/split/tests/test_*.py; do python3 "$t"; done
fi

printf 'fast ok: jain-split-ops\n'
