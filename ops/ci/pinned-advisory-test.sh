#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/pinned-advisory.sh
source "$repo_root/ops/ci/pinned-advisory.sh"

tmp="$(mktemp -d /tmp/jain-pinned-advisory-test.XXXXXX)"
trap 'rm -rf "$tmp"' EXIT

source_db="$tmp/source-db"
mkdir -p "$source_db/crates/example"
git -C "$source_db" init -q
git -C "$source_db" config user.name test
git -C "$source_db" config user.email test@example.invalid
printf 'id = "RUSTSEC-2099-0001"\n' >"$source_db/crates/example/RUSTSEC-2099-0001.md"
git -C "$source_db" add .
git -C "$source_db" commit -qm 'test advisory'
expected="$(git -C "$source_db" rev-parse HEAD)"

# Build a hostile mode-120000 tree entirely through Git's object/index APIs.
# No filesystem symbolic link is created even as a test fixture.
mode_source="$tmp/prohibited-mode-source"
mode_destination="$tmp/prohibited-mode-destination"
git init --quiet "$mode_source"
git -C "$mode_source" config user.name test
git -C "$mode_source" config user.email test@example.invalid
printf 'never materialize this target\n' >"$mode_source/blob-source"
mode_blob="$(git -C "$mode_source" hash-object -w blob-source)"
git -C "$mode_source" update-index --add \
  --cacheinfo "120000,$mode_blob,prohibited-link"
mode_tree="$(git -C "$mode_source" write-tree)"
mode_commit="$(git -C "$mode_source" commit-tree "$mode_tree" \
  -m 'prohibited symlink tree')"
if jain_git_object_tree_is_symlink_free "$mode_source" "$mode_commit"; then
  printf 'object-tree preflight accepted mode 120000\n' >&2
  exit 1
fi
if jain_materialize_pinned_advisory_db \
  "$mode_source" "$mode_destination" "$mode_commit"; then
  printf 'pinned advisory materialized mode 120000\n' >&2
  exit 1
fi
[[ ! -e "$mode_source/prohibited-link" \
  && ! -e "$mode_destination/prohibited-link" \
  && -z "$(find "$mode_source" "$mode_destination" -type l -print -quit 2>/dev/null)" ]] \
  || {
    printf 'object-only hostile fixture created a symbolic link\n' >&2
    exit 1
  }

# A user's source checkout may be dirty. Materialization must read the pinned
# commit without resetting, cleaning, deleting, or copying those changes.
printf 'user change\n' >>"$source_db/crates/example/RUSTSEC-2099-0001.md"
printf 'user untracked\n' >"$source_db/user-note"
source_status_before="$(git -C "$source_db" status --porcelain --untracked-files=all)"

advisory_db="$tmp/cargo-home/advisory-db"
jain_materialize_pinned_advisory_db "$source_db" "$advisory_db" "$expected"
[[ "$(git -C "$advisory_db" rev-parse HEAD)" == "$expected" ]] || exit 1
[[ -z "$(git -C "$advisory_db" status --porcelain --untracked-files=all)" ]] || exit 1
[[ "$(git -C "$source_db" status --porcelain --untracked-files=all)" == "$source_status_before" ]] || {
  printf 'pinned advisory materialization mutated the source checkout\n' >&2
  exit 1
}

# A caller-controlled partial clone can name an upload-pack command in local
# Git config. Root staging must neither execute it nor lazily fetch a missing
# object: source Git runs as the non-root owner with all transports disabled.
if [[ "${JAIN_HOST_CI_NETWORK_ISOLATED:-0}" != 1 ]]; then
promisor_source="$tmp/promisor-source"
promisor_remote="$tmp/promisor-remote.git"
promisor_destination="$tmp/promisor-destination"
uploadpack_marker="$tmp/uploadpack-executed"
uploadpack_attack="$tmp/malicious-uploadpack"
mkdir -p "$promisor_source/crates/promisor"
git -C "$promisor_source" init -q
git -C "$promisor_source" config user.name test
git -C "$promisor_source" config user.email test@example.invalid
printf 'promisor payload\n' >"$promisor_source/crates/promisor/advisory.md"
git -C "$promisor_source" add .
git -C "$promisor_source" commit -qm 'promisor fixture'
promisor_expected="$(git -C "$promisor_source" rev-parse HEAD)"
promisor_blob="$(git -C "$promisor_source" rev-parse \
  HEAD:crates/promisor/advisory.md)"
git clone --quiet --bare --no-hardlinks "$promisor_source" "$promisor_remote"
{
  printf '#!/bin/sh\n'
  printf ': >%q\n' "$uploadpack_marker"
  printf 'exec /usr/bin/git-upload-pack "$@"\n'
} >"$uploadpack_attack"
chmod 0700 "$uploadpack_attack"
git -C "$promisor_source" config core.repositoryFormatVersion 1
git -C "$promisor_source" config extensions.partialClone origin
git -C "$promisor_source" config remote.origin.url "$promisor_remote"
git -C "$promisor_source" config remote.origin.promisor true
git -C "$promisor_source" config remote.origin.partialCloneFilter blob:none
git -C "$promisor_source" config remote.origin.uploadpack "$uploadpack_attack"
rm -- "$promisor_source/.git/objects/${promisor_blob:0:2}/${promisor_blob:2}"
if sudo -n /bin/bash -ceu '
  source "$1"
  jain_materialize_pinned_advisory_db \
    "$2" "$3" "$4" "$5" "$6"
' -- "$repo_root/ops/ci/pinned-advisory.sh" \
  "$promisor_source" "$promisor_destination" "$promisor_expected" \
  "$(id -u)" "$(id -g)" >"$tmp/promisor-root.log" 2>&1; then
  printf 'pinned advisory accepted a source with a missing promised object\n' >&2
  exit 1
fi
[[ ! -e "$uploadpack_marker" ]] || {
  printf 'pinned advisory executed caller upload-pack config\n' >&2
  exit 1
}
sudo -n rm -rf -- "$promisor_destination"
else
  printf 'pinned advisory root staging covered by isolated broker boundary\n'
fi

tool_dir="$tmp/tools"
deny_db="$tmp/cargo-home/advisory-dbs/$JAIN_CARGO_DENY_RUSTSEC_DIR"
jain_materialize_pinned_advisory_db "$source_db" "$deny_db" "$expected"
jain_install_pinned_rustsec_tools "$tool_dir" "$advisory_db" \
  "$tmp/cargo-home" "$repo_root"
[[ -d "$deny_db/.git" && ! -L "$deny_db" && ! -L "$deny_db/.git" ]] || exit 1
[[ -f "$tool_dir/cargo-audit" && ! -L "$tool_dir/cargo-audit" \
  && -f "$tool_dir/cargo-deny" && ! -L "$tool_dir/cargo-deny" ]] || {
  printf 'pinned advisory tools were not installed as direct files\n' >&2
  exit 1
}

recorder="$repo_root/ops/ci/test-fixtures/rustsec-tool-recorder"
export JAIN_REAL_CARGO_AUDIT="$recorder"
export JAIN_REAL_CARGO_DENY="$recorder"
export JAIN_PINNED_ADVISORY_DB="$advisory_db"
export JAIN_ADVISORY_DB="$advisory_db"
export JAIN_CARGO_DENY_ADVISORY_DB="$deny_db"
export JAIN_PINNED_ADVISORY_COMMIT="$expected"
export JAIN_TEST_TOOL_ARGS="$tmp/args"

"$tool_dir/cargo-audit" audit --deny warnings --db /shared/user/db --no-fetch
mapfile -t args <"$JAIN_TEST_TOOL_ARGS"
[[ "${args[*]}" == "audit --deny warnings --db $advisory_db --no-fetch" ]] || {
  printf 'cargo-audit pin wrapper arguments drifted: %s\n' "${args[*]}" >&2
  exit 1
}

"$tool_dir/cargo-deny" deny check advisories --disable-fetch
mapfile -t args <"$JAIN_TEST_TOOL_ARGS"
[[ "${args[*]}" == "deny check --disable-fetch advisories" ]] || {
  printf 'cargo-deny pin wrapper arguments drifted: %s\n' "${args[*]}" >&2
  exit 1
}

export JAIN_CARGO_DENY_ADVISORY_DB="$deny_db/../$JAIN_CARGO_DENY_RUSTSEC_DIR"
if "$tool_dir/cargo-deny" deny check advisories >/dev/null 2>&1; then
  printf 'cargo-deny accepted an aliased advisory database spelling\n' >&2
  exit 1
fi
export JAIN_CARGO_DENY_ADVISORY_DB="$deny_db"

printf 'tamper\n' >"$advisory_db/untracked-tamper"
if "$tool_dir/cargo-audit" audit 2>/dev/null; then
  printf 'cargo-audit pin wrapper accepted a dirty database\n' >&2
  exit 1
fi

printf 'pinned advisory database contract ok\n'
