#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

for path in Cargo.toml Cargo.lock crates; do
  if [[ -e "$repo_root/$path" ]]; then
    printf 'redline hub contains forbidden engine path: %s\n' "$path" >&2
    exit 1
  fi
done

if git -C "$repo_root" ls-files | grep -E '(^|/)(kernel|sql|redlinedb)(/|$)' >/dev/null; then
  printf 'redline hub tracks engine source; move it to redline-core\n' >&2
  exit 1
fi

while IFS= read -r path; do
  [[ -f "$repo_root/$path" ]] || continue
  if grep -nE '(^|[[:space:]])(redlinedb|redlinedb-kernel|redlinedb-sql)[[:space:]]*=' "$repo_root/$path" >/dev/null; then
    printf 'redline hub declares an engine dependency in %s\n' "$path" >&2
    exit 1
  fi
done < <(git -C "$repo_root" ls-files '*.toml' '*.rs')

printf 'redline hub duplicate-engine guard: ok\n'
