#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"

fixture_root="$(mktemp -d "${TMPDIR:-/tmp}/redline-testing-rustsec.XXXXXX")"
cleanup() {
    rm -rf -- "$fixture_root"
}
trap cleanup EXIT

db="$fixture_root/db"
git init -q "$db"
git -C "$db" config user.name redline-testing-ci
git -C "$db" config user.email redline-testing-ci@invalid
printf 'fixture\n' >"$db/advisory.md"
git -C "$db" add advisory.md
git -C "$db" commit -q -m fixture
head="$(git -C "$db" rev-parse HEAD)"
tree="$(git -C "$db" rev-parse 'HEAD^{tree}')"
verify_rustsec_db_identity "$db" "$head" "$tree"

export JAIN_PINNED_ADVISORY_DB="$db"
export JAIN_ADVISORY_DB=
[ "$(governed_advisory_db_path)" = "$db" ]
export JAIN_PINNED_ADVISORY_DB=
export JAIN_ADVISORY_DB="$db"

export JAIN_PINNED_ADVISORY_COMMIT=0000000000000000000000000000000000000000
if governed_advisory_db_path >/dev/null 2>&1; then
    printf 'expected conflicting governed advisory commit to be rejected\n' >&2
    exit 1
fi
unset JAIN_PINNED_ADVISORY_COMMIT
[ "$(governed_advisory_db_path)" = "$db" ]
export JAIN_PINNED_ADVISORY_DB="$db"
[ "$(governed_advisory_db_path)" = "$db" ]

other_db="$fixture_root/other-db"
mkdir "$other_db"
export JAIN_ADVISORY_DB="$other_db"
if governed_advisory_db_path >/dev/null 2>&1; then
    printf 'expected conflicting governed advisory paths to be rejected\n' >&2
    exit 1
fi
export JAIN_ADVISORY_DB="$db"

expect_rejected() {
    local label="$1"
    shift
    if verify_rustsec_db_identity "$@" >/dev/null 2>&1; then
        printf 'expected RustSec database rejection: %s\n' "$label" >&2
        exit 1
    fi
}

mark_policy_command_started() {
    : >"${1:?policy command marker is required}"
}

expect_rejected missing "$fixture_root/missing" "$head" "$tree"

linked="$fixture_root/linked"
ln -s "$db" "$linked"
expect_rejected symlink "$linked" "$head" "$tree"

expect_rejected wrong-commit "$db" 0000000000000000000000000000000000000000 "$tree"
expect_rejected wrong-tree "$db" "$head" 0000000000000000000000000000000000000000
expect_rejected short-commit "$db" deadbeef "$tree"
expect_rejected short-tree "$db" "$head" deadbeef

printf 'dirty\n' >"$db/untracked"
expect_rejected dirty "$db" "$head" "$tree"
rm -f "$db/untracked"

ln -s advisory.md "$db/linked-advisory"
expect_rejected nested-symlink "$db" "$head" "$tree"
rm -f "$db/linked-advisory"

mkdir -p "$db/.git/objects/info"
printf '/hostile/objects\n' >"$db/.git/objects/info/alternates"
expect_rejected object-alternate "$db" "$head" "$tree"
rm -f "$db/.git/objects/info/alternates"

deny_home="$fixture_root/deny-home"
mkdir -p "$deny_home/advisory-dbs"
deny_db="$deny_home/advisory-dbs/$CARGO_DENY_RUSTSEC_DIR"
git clone -q --no-local --no-checkout "$db" "$deny_db"
git -C "$deny_db" checkout -q --detach "$head"
export JAIN_CARGO_DENY_ADVISORY_DB="$deny_db"
[ "$(governed_cargo_deny_db_path "$deny_home")" = "$deny_db" ]
binding_identity="$(
    cargo_deny_db_binding_identity "$deny_home" "$db" "$head" "$tree"
)"
verify_cargo_deny_db_binding_unchanged \
    "$binding_identity" "$deny_home" "$db" "$head" "$tree"

mv "$deny_db" "$fixture_root/original-deny-db"
ln -s "$db" "$deny_db"
if verify_cargo_deny_db_binding \
    "$deny_home" "$db" "$head" "$tree" >/dev/null 2>&1
then
    printf 'expected correct-target cargo-deny symlink to be rejected\n' >&2
    exit 1
fi
rm -f "$deny_db"
mv "$fixture_root/original-deny-db" "$deny_db"

parent_link_home="$fixture_root/parent-link-home"
real_advisory_parent="$fixture_root/real-advisory-parent"
mkdir "$parent_link_home" "$real_advisory_parent"
git clone -q --no-local --no-checkout \
    "$db" "$real_advisory_parent/$CARGO_DENY_RUSTSEC_DIR"
git -C "$real_advisory_parent/$CARGO_DENY_RUSTSEC_DIR" \
    checkout -q --detach "$head"
ln -s "$real_advisory_parent" "$parent_link_home/advisory-dbs"
if verify_cargo_deny_db_binding \
    "$parent_link_home" "$db" "$head" "$tree" >/dev/null 2>&1
then
    printf 'expected symlinked cargo-deny advisory parent to be rejected\n' >&2
    exit 1
fi

mv "$deny_db" "$fixture_root/pre-wrong-target-deny-db"
ln -s "$other_db" "$deny_db"
if verify_cargo_deny_db_binding \
    "$deny_home" "$db" "$head" "$tree" >/dev/null 2>&1
then
    printf 'expected wrong cargo-deny database binding to be rejected\n' >&2
    exit 1
fi
rm -f "$deny_db"
mv "$fixture_root/pre-wrong-target-deny-db" "$deny_db"

replacement_db="$fixture_root/replacement-deny-db"
git clone -q --no-local --no-checkout "$db" "$replacement_db"
git -C "$replacement_db" checkout -q --detach "$head"
mv "$deny_db" "$fixture_root/pre-replacement-deny-db"
mv "$replacement_db" "$deny_db"
verify_cargo_deny_db_binding "$deny_home" "$db" "$head" "$tree"
if verify_cargo_deny_db_binding_unchanged \
    "$binding_identity" "$deny_home" "$db" "$head" "$tree" >/dev/null 2>&1
then
    printf 'expected physical cargo-deny database replacement to be rejected\n' >&2
    exit 1
fi

transient_replacement_db="$fixture_root/transient-replacement-deny-db"
mv "$deny_db" "$transient_replacement_db"
mv "$fixture_root/pre-replacement-deny-db" "$deny_db"
verify_cargo_deny_db_binding_unchanged \
    "$binding_identity" "$deny_home" "$db" "$head" "$tree"

export JAIN_CARGO_DENY_ADVISORY_DB="$db"
if governed_cargo_deny_db_path "$deny_home" >/dev/null 2>&1; then
    printf 'expected an off-location cargo-deny database to be rejected\n' >&2
    exit 1
fi
export JAIN_CARGO_DENY_ADVISORY_DB="$deny_db"

# Reproduce the exact swap-and-restore gap in a plain worker-owned Cargo home:
# a separately cloned same-commit database is visible during the simulated
# scan, while the original inode is restored before the old post-check.
scan_started="$fixture_root/transient-scan-started"
swap_visible="$fixture_root/transient-swap-visible"
scan_release="$fixture_root/transient-scan-release"
transient_original_db="$fixture_root/transient-original-deny-db"
(
    while [ ! -e "$scan_started" ]; do sleep 0.01; done
    mv "$deny_db" "$transient_original_db"
    mv "$transient_replacement_db" "$deny_db"
    : >"$swap_visible"
    while [ ! -e "$scan_release" ]; do sleep 0.01; done
    mv "$deny_db" "$transient_replacement_db"
    mv "$transient_original_db" "$deny_db"
) &
swapper_pid=$!
: >"$scan_started"
while [ ! -e "$swap_visible" ]; do sleep 0.01; done
verify_cargo_deny_db_binding "$deny_home" "$db" "$head" "$tree"
: >"$scan_release"
wait "$swapper_pid"
verify_cargo_deny_db_binding_unchanged \
    "$binding_identity" "$deny_home" "$db" "$head" "$tree"

# Release CI must reject that mutable topology before the policy command can
# start. The real fleet positive uses a root-owned read-only bind mount; this
# hostile fixture deliberately is neither a mountpoint nor immutable custody.
forbidden_scan_marker="$fixture_root/forbidden-policy-command-start"
if JAIN_HOST_CI_NETWORK_ISOLATED=1 \
    run_with_cargo_deny_db_custody \
        "$deny_home" "$db" "$head" "$tree" \
        mark_policy_command_started "$forbidden_scan_marker" >/dev/null 2>&1
then
    printf 'expected mutable isolated cargo-deny custody to be rejected\n' >&2
    exit 1
fi
[ ! -e "$forbidden_scan_marker" ] || {
    printf 'dependency-policy command started before immutable custody proof\n' >&2
    exit 1
}
if verify_cargo_deny_db_immutable_custody_for_owner \
    "$deny_home" "$db" "$head" "$tree" "$(id -u)" >/dev/null 2>&1
then
    printf 'expected a renameable non-mount advisory parent to be rejected\n' >&2
    exit 1
fi
unset JAIN_CARGO_DENY_ADVISORY_DB
if JAIN_HOST_CI_NETWORK_ISOLATED=1 \
    governed_cargo_deny_db_path "$deny_home" >/dev/null 2>&1
then
    printf 'expected isolated CI without an explicit deny DB to be rejected\n' >&2
    exit 1
fi

physical_home="$fixture_root/physical-home"
mkdir -p "$physical_home/advisory-dbs"
git clone -q --no-local --no-checkout \
    "$db" "$physical_home/advisory-dbs/$CARGO_DENY_RUSTSEC_DIR"
git -C "$physical_home/advisory-dbs/$CARGO_DENY_RUSTSEC_DIR" \
    checkout -q --detach "$head"
home_link="$fixture_root/home-link"
ln -s "$physical_home" "$home_link"
if verify_cargo_deny_db_binding \
    "$home_link" "$db" "$head" "$tree" >/dev/null 2>&1
then
    printf 'expected symlinked cargo home to be rejected\n' >&2
    exit 1
fi

fixture_manifest="$fixture_root/incomplete/Cargo.toml"
mkdir -p "$(dirname "$fixture_manifest")" "$fixture_root/empty-cargo-home"
cat >"$fixture_manifest" <<'EOF'
[package]
name = "incomplete-cache-probe"
version = "0.0.0"
edition = "2021"

[dependencies]
id-arena = "=2.3.0"
EOF
cat >"${fixture_manifest%/*}/Cargo.lock" <<'EOF'
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "id-arena"
version = "2.3.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "3d3067d79b975e8844ca9eb072e16b31c3c1c36928edf9c6789548c524d0d954"

[[package]]
name = "incomplete-cache-probe"
version = "0.0.0"
dependencies = [
 "id-arena",
]
EOF
if CARGO_HOME="$fixture_root/empty-cargo-home" \
    verify_locked_cargo_closure "$fixture_manifest" >/dev/null 2>&1; then
    printf 'expected incomplete offline Cargo cache to be rejected\n' >&2
    exit 1
fi

printf 'governed security input negative probes ok\n'
