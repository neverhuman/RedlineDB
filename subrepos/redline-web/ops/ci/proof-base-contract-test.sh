#!/usr/bin/env bash
# Prove proof-routing and proofbind remain usable in the governed no-remote
# checkout while rejecting every non-commit release base representation.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"

has git || fail "git is required for the proof-base contract test"
head_commit="$(git rev-parse --verify 'HEAD^{commit}')"
if [[ "${JAIN_RELEASE_CI:-0}" == "1" ]]; then
  : "${JAIN_CONTRACT_BASE_REF:?release proof base commit is required}"
  source_base="$JAIN_CONTRACT_BASE_REF"
else
  source_base="${JAIN_CONTRACT_BASE_REF:-refs/remotes/origin/main}"
fi
base_commit="$(git rev-parse --verify "${source_base}^{commit}" 2>/dev/null)" \
  || fail "proof-base contract fixture base is unavailable: ${source_base}"
if [[ "$base_commit" == "$head_commit" ]]; then
  base_commit="$(git rev-parse --verify 'HEAD~1^{commit}' 2>/dev/null)" \
    || fail "proof-base contract fixture needs one ancestor"
fi
git merge-base --is-ancestor "$base_commit" "$head_commit" \
  || fail "proof-base contract fixture base is not an ancestor"

mkdir -p "${ROOT_DIR}/target"
test_root="$(mktemp -d "${ROOT_DIR}/target/proof-base-contract.XXXXXX")"
cleanup() {
  [[ "$test_root" == "${ROOT_DIR}"/target/proof-base-contract.* ]] \
    || fail "refusing unexpected proof-base fixture cleanup: ${test_root}"
  rm -rf -- "$test_root"
}
trap cleanup EXIT
git clone --quiet --no-local --no-checkout "$ROOT_DIR" "$test_root/source"
git -C "$test_root/source" checkout --quiet --detach "$head_commit"
git -C "$test_root/source" remote remove origin
[[ -z "$(git -C "$test_root/source" remote)" ]] \
  || fail "proof-base contract fixture retained a remote"
[[ -z "$(git -C "$test_root/source" status --porcelain=v1)" ]] \
  || fail "proof-base contract fixture is dirty"
cd "$test_root/source"

expect_failure() {
  local expected="${1:?expected message required}"
  shift
  local output status
  set +e
  output="$("$@" 2>&1)"
  status=$?
  set -e
  [[ "$status" -ne 0 && "$output" == *"$expected"* ]] \
    || fail "expected fail-closed proof-base error: ${expected}"
}

# Release mode ignores the developer override and consumes only the
# independently authenticated base object exported by the host.
JAIN_RELEASE_CI=1 JAIN_CONTRACT_BASE_REF="$base_commit" \
  JANKURAI_BASE_REF=0000000000000000000000000000000000000000 \
  bash ops/ci/governed-jankurai.sh proof . --changed-from "$base_commit" \
    --out target/jankurai/proof-routing.json \
    --md target/jankurai/proof-routing.md
JAIN_RELEASE_CI=1 JAIN_CONTRACT_BASE_REF="$base_commit" \
  JANKURAI_BASE_REF=0000000000000000000000000000000000000000 \
  bash ops/ci/proofbind.sh

for consumer in ops/ci/jankurai.sh ops/ci/proofbind.sh; do
  base_label="proof"
  [[ "$consumer" == ops/ci/proofbind.sh ]] && base_label="proofbind"
  expect_failure "release proof base commit is required" \
    env -u JAIN_CONTRACT_BASE_REF JAIN_RELEASE_CI=1 bash "$consumer"
  expect_failure "release proof base must be a full lowercase commit: main" \
    env JAIN_RELEASE_CI=1 JAIN_CONTRACT_BASE_REF=main bash "$consumer"
  expect_failure "${base_label} base is not a local commit: 0000000000000000000000000000000000000000" \
    env JAIN_RELEASE_CI=1 \
      JAIN_CONTRACT_BASE_REF=0000000000000000000000000000000000000000 \
      bash "$consumer"
done

nonancestor="$(
  printf 'nonancestor\n' \
    | GIT_AUTHOR_NAME=Jain GIT_AUTHOR_EMAIL=jain@invalid \
      GIT_COMMITTER_NAME=Jain GIT_COMMITTER_EMAIL=jain@invalid \
      git commit-tree 'HEAD^{tree}'
)"
tag_name="proof-base-contract-annotated"
GIT_COMMITTER_NAME=Jain GIT_COMMITTER_EMAIL=jain@invalid \
  git tag -a "$tag_name" "$base_commit" -m "hostile annotated base"
tag_object="$(git rev-parse "refs/tags/${tag_name}")"
for consumer in ops/ci/jankurai.sh ops/ci/proofbind.sh; do
  base_label="proof"
  [[ "$consumer" == ops/ci/proofbind.sh ]] && base_label="proofbind"
  expect_failure "${base_label} base is not an ancestor of HEAD: ${nonancestor}" \
    env JAIN_RELEASE_CI=1 JAIN_CONTRACT_BASE_REF="$nonancestor" \
      bash "$consumer"
  expect_failure \
    "release proof base must resolve to itself: ${tag_object} -> ${base_commit}" \
    env JAIN_RELEASE_CI=1 JAIN_CONTRACT_BASE_REF="$tag_object" \
      bash "$consumer"
done

# Developer runs keep the explicit local fallback outside release mode.
JANKURAI_BASE_REF="$base_commit" bash ops/ci/proofbind.sh
[[ -z "$(git status --porcelain=v1)" ]] \
  || fail "proof-base contract test mutated its checkout"
log "proof-base-contract: governed no-remote base routing passed"
