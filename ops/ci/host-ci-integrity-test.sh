#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tmp="$(mktemp -d /tmp/jain-host-ci-integrity-test.XXXXXX)"
trap 'rm -rf "$tmp"' EXIT
fixture="$tmp/control"
mkdir -p "$fixture/ops/ci" "$fixture/tools/splitctl/src"

cp -- "$repo_root/ops/ci/host-ci-integrity.sh" "$fixture/ops/ci/host-ci-integrity.sh"
for path in \
  Cargo.lock Cargo.toml repos.manifest.toml \
  ops/ci/native-runtime.sh ops/ci/pinned-advisory.sh \
  ops/ci/pinned-cargo-audit.sh ops/ci/pinned-cargo-deny.sh \
  ops/ci/split-host-ci.sh tools/splitctl/src/main.rs; do
  mkdir -p "$fixture/$(dirname "$path")"
  printf 'fixture %s\n' "$path" >"$fixture/$path"
done
chmod +x "$fixture/ops/ci/host-ci-integrity.sh"
git init --quiet "$fixture"
git -C "$fixture" config user.name 'Host CI Fixture'
git -C "$fixture" config user.email host-ci-fixture@example.invalid
git -C "$fixture" add .
git -C "$fixture" commit --quiet -m exact
fixture_commit="$(git -C "$fixture" rev-parse HEAD)"
[[ "$("$fixture/ops/ci/host-ci-integrity.sh" \
  "$fixture" "$fixture_commit")" == "$fixture_commit" ]] || exit 1
if "$fixture/ops/ci/host-ci-integrity.sh" "$fixture" \
  0123456789abcdef0123456789abcdef01234567 >/dev/null 2>&1; then
  printf 'host CI integrity accepted a different expected commit\n' >&2
  exit 1
fi

printf 'dirty runtime\n' >>"$fixture/ops/ci/native-runtime.sh"
if "$fixture/ops/ci/host-ci-integrity.sh" "$fixture" >/dev/null 2>&1; then
  printf 'host CI integrity accepted a dirty native runtime\n' >&2
  exit 1
fi
git -C "$fixture" restore ops/ci/native-runtime.sh

printf 'dirty runner\n' >>"$fixture/ops/ci/split-host-ci.sh"
if "$fixture/ops/ci/host-ci-integrity.sh" "$fixture" >/dev/null 2>&1; then
  printf 'host CI integrity accepted a dirty status runner\n' >&2
  exit 1
fi
git -C "$fixture" restore ops/ci/split-host-ci.sh

printf 'untracked orchestration\n' >"$fixture/ops/ci/unreviewed.sh"
if "$fixture/ops/ci/host-ci-integrity.sh" "$fixture" >/dev/null 2>&1; then
  printf 'host CI integrity accepted unreviewed control-plane bytes\n' >&2
  exit 1
fi
rm -- "$fixture/ops/ci/unreviewed.sh"

printf '# dirty integrity gate\n' >>"$fixture/ops/ci/host-ci-integrity.sh"
if "$fixture/ops/ci/host-ci-integrity.sh" "$fixture" >/dev/null 2>&1; then
  printf 'host CI integrity accepted its own dirty bytes\n' >&2
  exit 1
fi

printf 'host CI exact orchestration contract ok\n'
