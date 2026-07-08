#!/usr/bin/env bash
# Required lane for the jain-split-ops control-plane repo. This repo has no product crate;
# its "product" is the fleet tooling (shell CI runners + the Python materializer + the repo
# manifest), so the lane validates THOSE: shell parses + lints, Python compiles + self-tests,
# and the manifest is well-formed. Closes the "control plane is ungated" gap.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

say() { printf '[required:jain-split-ops] %s\n' "$*" >&2; }

say 'shell: bash -n + shellcheck (error level) on all ops shell scripts'
mapfile -t sh_files < <(find ops -type f -name '*.sh' | sort)
for f in "${sh_files[@]}"; do bash -n "$f"; done
if command -v shellcheck >/dev/null 2>&1; then
  # -S error: fail only on genuine errors, not pre-existing style warnings.
  shellcheck -S error "${sh_files[@]}"
else
  say 'shellcheck not installed; skipped lint (bash -n still ran)'
fi

say 'python: compile every control-plane module'
mapfile -t py_files < <(find ops -type f -name '*.py' | sort)
python3 -m py_compile "${py_files[@]}"

say 'python: materializer self-tests'
if python3 -c 'import pytest' 2>/dev/null; then
  python3 -m pytest -q ops/split/tests
else
  # Fall back to executing each test module directly if pytest is unavailable.
  for t in ops/split/tests/test_*.py; do python3 "$t"; done
fi

say 'manifest: repos.manifest.toml parses'
python3 - <<'PY'
import sys
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib  # py<3.11
with open('repos.manifest.toml', 'rb') as fh:
    data = tomllib.load(fh)
assert data, 'repos.manifest.toml is empty'
print(f'[required:jain-split-ops] manifest ok: {len(data)} top-level tables')
PY

printf 'required ok: jain-split-ops\n'
