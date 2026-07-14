#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tmp="$(mktemp -d /tmp/jain-split-host-integrity-test.XXXXXX)"
trap 'rm -rf "$tmp"' EXIT

control="$tmp/control"
control_remote="$tmp/jain-split-ops.git"
split_root="$tmp/split"
product="$split_root/jain-report"
fake_bin="$tmp/bin"
forge_log="$tmp/forge.log"
started="$tmp/started"
continue_file="$tmp/continue"
exact_root_file="$tmp/exact-root"
runner_log="$tmp/runner.log"

git clone --quiet --shared "$repo_root" "$control"
git -C "$control" config user.name 'Host CI Integration Fixture'
git -C "$control" config user.email host-ci-integration@example.invalid
git init --quiet --bare "$control_remote"
sed -i \
  "s#remote = \"http://127.0.0.1:8787/git/jeryu/jain-split-ops.git\"#remote = \"$control_remote\"#" \
  "$control/repos.manifest.toml"
git -C "$control" add repos.manifest.toml
git -C "$control" commit --quiet -m 'fixture authority remote'
git -C "$control" branch -M main
git -C "$control" remote set-url origin "$control_remote"
git -C "$control" push --quiet -u origin main

mkdir -p "$product/scripts" "$split_root/jain-core" "$fake_bin"
printf '%s\n' \
  '#!/usr/bin/env bash' \
  'set -euo pipefail' \
  ': "${JAIN_TEST_STARTED:?}" "${JAIN_TEST_CONTINUE:?}" "${JAIN_TEST_EXACT_ROOT_FILE:?}"' \
  'printf "%s\n" "${JAIN_HOST_CI_EXACT_ROOT:?}" >"$JAIN_TEST_EXACT_ROOT_FILE"' \
  'touch "$JAIN_TEST_STARTED"' \
  'while [[ ! -e "$JAIN_TEST_CONTINUE" ]]; do sleep 0.02; done' \
  >"$product/scripts/ci-local.sh"
git init --quiet "$product"
git -C "$product" config user.name 'Product Fixture'
git -C "$product" config user.email product-fixture@example.invalid
git -C "$product" add .
git -C "$product" commit --quiet -m fixture
product_sha="$(git -C "$product" rev-parse HEAD)"

# Executable fake forge: health succeeds and every check/status write is
# recorded verbatim for assertions below.
printf '%s\n' \
  '#!/usr/bin/env bash' \
  'set -euo pipefail' \
  'url=""; payload=""' \
  'while (($#)); do' \
  '  case "$1" in' \
  '    -d) payload="${2:?}"; shift 2 ;;' \
  '    http://* | https://*) url="$1"; shift ;;' \
  '    *) shift ;;' \
  '  esac' \
  'done' \
  '[[ "$url" == */health ]] && exit 0' \
  'printf "%s\t%s\n" "$url" "$payload" >>"${JAIN_TEST_FORGE_LOG:?}"' \
  >"$fake_bin/curl"
chmod +x "$fake_bin/curl"

PATH="$fake_bin:$PATH" \
JAIN_SPLIT_ROOT="$split_root" \
JAIN_BASE=http://fake-forge.invalid \
JERYU_MERGE_TOKEN=fixture-token \
JAIN_TEST_FORGE_LOG="$forge_log" \
JAIN_TEST_STARTED="$started" \
JAIN_TEST_CONTINUE="$continue_file" \
JAIN_TEST_EXACT_ROOT_FILE="$exact_root_file" \
CARGO_TARGET_DIR="$repo_root/target" \
  "$control/ops/ci/split-host-ci.sh" \
    jeryu jain-report "$product_sha" "$product" jain-report/required \
    >"$runner_log" 2>&1 &
runner_pid=$!

for _ in $(seq 1 500); do
  [[ -s "$exact_root_file" && -e "$started" ]] && break
  kill -0 "$runner_pid" 2>/dev/null || break
  sleep 0.02
done
[[ -s "$exact_root_file" && -e "$started" ]] || {
  cat "$runner_log" >&2
  printf 'host CI integration runner did not reach its governed lane\n' >&2
  exit 1
}
exact_root="$(<"$exact_root_file")"
[[ -d "$exact_root" ]] || exit 1
printf '# concurrent mutation\n' >>"$exact_root/ops/ci/native-runtime.sh"
touch "$continue_file"

if wait "$runner_pid"; then
  printf 'host CI published success after concurrent orchestration mutation\n' >&2
  exit 1
fi
grep -Fq 'control-plane bytes changed before success publication' "$runner_log" || {
  cat "$runner_log" >&2
  printf 'host CI mutation failure did not come from the final integrity gate\n' >&2
  exit 1
}
if grep -Fq '"conclusion":"success"' "$forge_log"; then
  printf 'fake forge received a success check for mutated orchestration\n' >&2
  exit 1
fi
if grep -Fq '"state":"success"' "$forge_log"; then
  printf 'fake forge received a success status for mutated orchestration\n' >&2
  exit 1
fi
grep -Fq '"conclusion":"failure"' "$forge_log" || {
  printf 'fake forge did not receive the mutation failure check\n' >&2
  exit 1
}
grep -Fq '"state":"failure"' "$forge_log" || {
  printf 'fake forge did not receive the mutation failure status\n' >&2
  exit 1
}

printf 'host CI exact-commit re-exec and concurrent-mutation contract ok\n'
