#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

out_dir="target/proof/sqlite-full-parity"
mkdir -p "$out_dir"

required_receipts=(
  git-status.txt
  diff-stat.txt
  rusqlite-reference.txt
  unsupported-sql-sites.txt
  ignored-tests.txt
  sqllogictest-inventory.txt
  sql-parity-tests.txt
)

{
  printf 'generated_at=%s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
  printf 'head=%s\n' "$(git rev-parse HEAD)"
  git status --short --branch
} > "$out_dir/git-status.txt"

{
  printf '# Unstaged diff stat\n'
  git diff --stat -- . || true
  printf '\n# Staged diff stat\n'
  git diff --cached --stat -- . || true
} > "$out_dir/diff-stat.txt"

{
  printf '# rusqlite dependency\n'
  cargo tree -p redlinedb-sql -i rusqlite --locked
  printf '\n# bundled SQLite metadata from rusqlite\n'
  cargo test -p redlinedb-sql --test sqlite_full_parity \
    reference_build_metadata_is_available --locked -- --nocapture
} > "$out_dir/rusqlite-reference.txt" 2>&1

{
  printf '# UnsupportedSql source sites\n'
  rg -n 'UnsupportedSql' crates/sql/src crates/sql/tests || true
} > "$out_dir/unsupported-sql-sites.txt"

{
  printf '# Ignored SQL/bench parity tests\n'
  rg -n '#\[ignore' crates/sql/tests crates/bench/tests || true
} > "$out_dir/ignored-tests.txt"

{
  printf '# SQLLogicTest inventory\n'
  find crates/bench/compat -type f \( -name '*.sqlt' -o -name '*.slt' \) -print \
    | sort \
    | while IFS= read -r file; do
        lines="$(wc -l < "$file")"
        directives="$(grep -Ec '^(statement|query)\b' "$file" || true)"
        printf '%s lines=%s directives=%s\n' "$file" "$lines" "$directives"
      done
} > "$out_dir/sqllogictest-inventory.txt"

{
  printf '# SQL parity test files\n'
  find crates/sql/tests -maxdepth 1 -type f \
    \( -name 'parity_*.rs' -o -name 'sqlite_full_parity.rs' -o -name 'differential_lab.rs' \) \
    -print \
    | sort \
    | while IFS= read -r file; do
        tests="$(grep -Ec '^#\[test\]' "$file" || true)"
        printf '%s tests=%s\n' "$file" "$tests"
      done
  printf '\n# parity_oracle corpus files\n'
  find crates/sql/tests/parity_corpus -type f -name '*.sql' -print | sort
} > "$out_dir/sql-parity-tests.txt"

missing=0
for receipt in "${required_receipts[@]}"; do
  if [[ ! -s "$out_dir/$receipt" ]]; then
    printf 'missing or empty receipt: %s/%s\n' "$out_dir" "$receipt" >&2
    missing=1
  fi
done

exit "$missing"
