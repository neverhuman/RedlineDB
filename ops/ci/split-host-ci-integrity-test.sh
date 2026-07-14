#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tmp="$(mktemp -d /tmp/jain-split-host-integrity-test.XXXXXX)"
forged_root=""
cleanup() {
  rm -rf -- "$tmp"
  case "$forged_root" in
    /tmp/split-host-ci-bootstrap.??????) rm -rf -- "$forged_root" ;;
  esac
}
trap cleanup EXIT

control="$tmp/control"
control_remote="$tmp/jain-split-ops.git"
split_root="$tmp/split"
product="$split_root/jain-report"
fake_bin="$tmp/bin"
forge_log="$tmp/forge.log"
started="$tmp/started"
continue_file="$tmp/continue"
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
git -C "$control" switch -C main --quiet
git -C "$control" remote set-url origin "$control_remote"
git -C "$control" push --quiet -u origin main

mkdir -p "$product/scripts" "$split_root/jain-core" "$fake_bin"
printf '%s\n' \
  '#!/usr/bin/env bash' \
  'set -euo pipefail' \
  ': "${JAIN_TEST_STARTED:?}" "${JAIN_TEST_CONTINUE:?}"' \
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

# Reproduce the rejected direct-entry bypass exactly: legacy caller-provided
# mode markers must not skip public bootstrap, and its chosen cleanup victim
# must survive even when the requested product repository is invalid.
victim="$tmp/codex-preseed-victim"
mkdir -p "$victim"
printf 'preserve\n' >"$victim/sentinel"
if JAIN_HOST_CI_EXACT_ROOT="$control" \
  JAIN_HOST_CI_SOURCE_ROOT="$control" \
  JAIN_HOST_CI_CONTROL_COMMIT="$(git -C "$control" rev-parse HEAD)" \
  JAIN_HOST_CI_BOOTSTRAP_ROOT="$victim" \
  JAIN_SPLIT_ROOT="$split_root" \
    "$control/ops/ci/split-host-ci.sh" \
      jeryu jain-report "$product_sha" "$tmp/not-a-repository" \
      jain-report/required >/dev/null 2>&1; then
  printf 'host CI accepted an invalid product through preseeded mode markers\n' >&2
  exit 1
fi
[[ "$(<"$victim/sentinel")" == preserve ]] || {
  printf 'host CI deleted a caller-selected bootstrap victim\n' >&2
  exit 1
}

# Calling the reviewed filename directly with a forged sealed state also fails
# closed and performs no cleanup. This exercises the internal entrypoint rather
# than merely inspecting its source.
forged_root="$(mktemp -d /tmp/split-host-ci-bootstrap.XXXXXX)"
chmod 0700 "$forged_root"
cp -- "$control/ops/ci/split-host-ci.sh" \
  "$forged_root/.split-host-ci-reviewed"
chmod 0500 "$forged_root/.split-host-ci-reviewed"
forged_seal="$(printf 'ab%.0s' {1..32})"
jq -n --arg seal "$forged_seal" --arg source_root "$control" \
  --arg exact_root "$victim" \
  --arg commit "$(git -C "$control" rev-parse HEAD)" \
  '{schema_version:"jain.host-ci-reexec/v1",seal:$seal,
    source_root:$source_root,exact_root:$exact_root,commit:$commit}' \
  >"$forged_root/reexec-state.json"
chmod 0600 "$forged_root/reexec-state.json"
if JAIN_HOST_CI_REEXEC_STATE="$forged_root/reexec-state.json" \
  JAIN_HOST_CI_REEXEC_SEAL="$forged_seal" \
    bash "$forged_root/.split-host-ci-reviewed" \
      jeryu jain-report "$product_sha" "$product" jain-report/required \
      >/dev/null 2>&1; then
  printf 'host CI accepted a forged reviewed-runner state\n' >&2
  exit 1
fi
[[ -s "$victim/sentinel" && -d "$forged_root" ]] || {
  printf 'forged reviewed entry deleted caller-controlled paths\n' >&2
  exit 1
}
rm -rf -- "$forged_root"

PATH="$fake_bin:$PATH" \
JAIN_SPLIT_ROOT="$split_root" \
JAIN_BASE=http://fake-forge.invalid \
JERYU_MERGE_TOKEN=fixture-token \
JAIN_TEST_FORGE_LOG="$forge_log" \
JAIN_TEST_STARTED="$started" \
JAIN_TEST_CONTINUE="$continue_file" \
CARGO_TARGET_DIR="$repo_root/target" \
  "$control/ops/ci/split-host-ci.sh" \
    jeryu jain-report "$product_sha" "$product" jain-report/required \
    >"$runner_log" 2>&1 &
runner_pid=$!

for _ in $(seq 1 500); do
  [[ -e "$started" ]] && break
  kill -0 "$runner_pid" 2>/dev/null || break
  sleep 0.02
done
[[ -e "$started" ]] || {
  cat "$runner_log" >&2
  printf 'host CI integration runner did not reach its governed lane\n' >&2
  exit 1
}
exact_root=""
while IFS= read -r worktree; do
  case "$worktree" in
    /tmp/split-host-ci-bootstrap.??????/control-plane)
      exact_root="$worktree"
      ;;
  esac
done < <(git -C "$control" worktree list --porcelain \
  | sed -n 's/^worktree //p')
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
