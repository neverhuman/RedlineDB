#!/usr/bin/env bash
# ShellCheck treats intentional hostile-case subshell environments as lost assignments.
# shellcheck disable=SC2030,SC2031
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

# shellcheck source=ops/ci/pinned-rustsec.sh
source ops/ci/pinned-rustsec.sh

tmp="$(mktemp -d "${TMPDIR:-/tmp}/redline-central-rustsec-test.XXXXXX")"
trap 'rm -rf -- "$tmp"' EXIT
fixture="$tmp/advisory-db"
mkdir -p "$fixture/crates"
printf '[advisories]\n' >"$fixture/support.toml"
printf 'fixture\n' >"$fixture/crates/README.md"
git -C "$fixture" init -q
git -C "$fixture" add support.toml crates/README.md
git -c user.name=fixture -c user.email=fixture@example.invalid \
  -C "$fixture" commit -q -m fixture
commit="$(git -C "$fixture" rev-parse 'HEAD^{commit}')"
tree="$(git -C "$fixture" rev-parse 'HEAD^{tree}')"

if (export JAIN_RELEASE_CI=1; unset JAIN_PINNED_ADVISORY_DB JAIN_PINNED_ADVISORY_COMMIT;
    jain_resolve_governed_rustsec) >/dev/null 2>&1; then
  printf 'release CI accepted missing governed advisory variables\n' >&2
  exit 1
fi
if (export JAIN_PINNED_ADVISORY_DB="$fixture"; unset JAIN_PINNED_ADVISORY_COMMIT;
    jain_resolve_governed_rustsec) >/dev/null 2>&1; then
  printf 'a partial governed advisory identity was accepted\n' >&2
  exit 1
fi
if (export JAIN_PINNED_ADVISORY_DB="$fixture/missing"
    JAIN_PINNED_ADVISORY_COMMIT="$commit"; jain_resolve_governed_rustsec) \
    >/dev/null 2>&1; then
  printf 'a missing advisory database was accepted\n' >&2
  exit 1
fi
ln -s -- "$fixture" "$tmp/linked-db"
if (export JAIN_PINNED_ADVISORY_DB="$tmp/linked-db"
    JAIN_PINNED_ADVISORY_COMMIT="$commit"; jain_resolve_governed_rustsec) \
    >/dev/null 2>&1; then
  printf 'a linked advisory database was accepted\n' >&2
  exit 1
fi
if (export JAIN_PINNED_ADVISORY_DB="$fixture"
    JAIN_PINNED_ADVISORY_COMMIT=0000000000000000000000000000000000000000;
    jain_resolve_governed_rustsec) >/dev/null 2>&1; then
  printf 'an advisory commit mismatch was accepted\n' >&2
  exit 1
fi
printf 'dirty\n' >"$fixture/untracked"
if (export JAIN_PINNED_ADVISORY_DB="$fixture"
    JAIN_PINNED_ADVISORY_COMMIT="$commit"; jain_resolve_governed_rustsec) \
    >/dev/null 2>&1; then
  printf 'a dirty advisory database was accepted\n' >&2
  exit 1
fi
rm -- "$fixture/untracked"

(
  export JAIN_PINNED_ADVISORY_DB="$fixture"
  export JAIN_PINNED_ADVISORY_COMMIT="$commit"
  jain_resolve_governed_rustsec
  [[ "$JAIN_RESOLVED_ADVISORY_DB" == "$fixture" ]]
  [[ "$JAIN_RESOLVED_ADVISORY_COMMIT" == "$commit" ]]
  [[ "$JAIN_RESOLVED_ADVISORY_TREE" == "$tree" ]]
  [[ "$JAIN_RESOLVED_ADVISORY_AUTHORITY" == governed_host ]]
)

printf 'governed RustSec contract ok: exact path, commit, tree, and cleanliness\n'
