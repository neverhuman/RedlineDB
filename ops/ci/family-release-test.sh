#!/usr/bin/env bash
# ShellCheck treats intentional hostile-case subshell environments as lost assignments.
# shellcheck disable=SC2030,SC2031
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

tmp="$(mktemp -d "${TMPDIR:-/tmp}/redline-central-family-release.XXXXXX")"
trap 'rm -rf -- "$tmp"' EXIT
mkdir -m 0700 "$tmp/bin"

cat >"$tmp/bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\t%s\t%s\n' "$DB_DSN" "$DB_NAMESPACE" "$*" >>"$FAMILY_RELEASE_TEST_LOG"
EOF
chmod 0700 "$tmp/bin/cargo"

run_lane() {
  PATH="$tmp/bin:$PATH" FAMILY_RELEASE_TEST_LOG="$tmp/cargo.log" \
    bash ops/ci/family-release.sh
}

if (unset REDLINE_CORPUS_DSN POSTGRES_CORPUS_DSN; run_lane) >/dev/null 2>&1; then
  printf 'family-release accepted missing service DSNs\n' >&2
  exit 1
fi
if (export REDLINE_CORPUS_DSN='redline://redline.example:6033';
    unset POSTGRES_CORPUS_DSN; run_lane) >/dev/null 2>&1; then
  printf 'family-release accepted a missing Postgres DSN\n' >&2
  exit 1
fi
if (export REDLINE_CORPUS_DSN=:memory:
    POSTGRES_CORPUS_DSN='postgres://postgres.example/redline'; run_lane) >/dev/null 2>&1; then
  printf 'family-release accepted an in-process Redline substitute\n' >&2
  exit 1
fi
if (export REDLINE_CORPUS_DSN='redline://redline.example:6033'
    POSTGRES_CORPUS_DSN='sqlite:///tmp/not-postgres'; run_lane) >/dev/null 2>&1; then
  printf 'family-release accepted a non-Postgres substitute\n' >&2
  exit 1
fi

export REDLINE_CORPUS_DSN='redline://redline.example:6033'
export POSTGRES_CORPUS_DSN='postgres://postgres.example/redline'
run_lane >/dev/null
[[ "$(wc -l <"$tmp/cargo.log")" -eq 2 ]]
grep -Fq $'redline://redline.example:6033\tfamily_release_redline\trun --locked -p db-shim --no-default-features --features backend-redline --bin db-shim-parity' \
  "$tmp/cargo.log"
grep -Fq $'postgres://postgres.example/redline\tfamily_release_postgres\trun --locked -p db-shim --no-default-features --features oracle-postgres --bin db-shim-parity' \
  "$tmp/cargo.log"

printf 'family-release contract ok: explicit services required and forwarded exactly\n'
