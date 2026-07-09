#!/usr/bin/env bash
set -euo pipefail

for tool in bash git just python3 shellcheck jankurai; do
  printf '%s: ' "$tool"
  if command -v "$tool" >/dev/null 2>&1; then
    command -v "$tool"
  else
    printf 'missing\n'
  fi
done

