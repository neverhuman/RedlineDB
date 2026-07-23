#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/pinned-rustsec.sh
source "$repo_root/ops/ci/pinned-rustsec.sh"

tmp="$(mktemp -d "${TMPDIR:-/tmp}/redline-core-rustsec.XXXXXX")"
trap 'rm -rf -- "$tmp"' EXIT
source_db="${JAIN_RUSTSEC_ADVISORY_SOURCE:-${HOME:?HOME is required}/.cargo/advisory-db}"
db="$tmp/advisory-db"
git clone -q --no-local --no-checkout "$source_db" "$db"
git -C "$db" checkout -q --detach "$REDLINE_RUSTSEC_COMMIT"

(
    unset JAIN_PINNED_ADVISORY_DB JAIN_ADVISORY_DB JAIN_PINNED_ADVISORY_COMMIT
    export JAIN_RUSTSEC_ADVISORY_SOURCE="$db"
    redline_resolve_pinned_rustsec
    [[ "$REDLINE_PINNED_ADVISORY_DB" == "$db" ]]
)
expect_rejected() {
    local label="$1"
    shift
    if ("$@") >/dev/null 2>&1; then
        printf 'RustSec input unexpectedly accepted: %s\n' "$label" >&2
        exit 1
    fi
}

missing_release() {
    export JAIN_RELEASE_CI=1
    unset JAIN_PINNED_ADVISORY_DB JAIN_ADVISORY_DB JAIN_PINNED_ADVISORY_COMMIT
    redline_resolve_pinned_rustsec
}
wrong_commit() {
    export JAIN_PINNED_ADVISORY_DB="$db" JAIN_ADVISORY_DB="$db"
    export JAIN_PINNED_ADVISORY_COMMIT=0000000000000000000000000000000000000000
    redline_resolve_pinned_rustsec
}
unsealed_release() {
    export JAIN_RELEASE_CI=1
    export JAIN_PINNED_ADVISORY_DB="$db"
    export JAIN_ADVISORY_DB="$db"
    export JAIN_PINNED_ADVISORY_COMMIT="$REDLINE_RUSTSEC_COMMIT"
    redline_resolve_pinned_rustsec
}
conflicting_paths() {
    export JAIN_PINNED_ADVISORY_DB="$db" JAIN_ADVISORY_DB="$tmp/other"
    redline_resolve_pinned_rustsec
}

expect_rejected missing-release missing_release
expect_rejected wrong-commit wrong_commit
expect_rejected conflicting-paths conflicting_paths
expect_rejected unsealed-release unsealed_release

hostile_home="$tmp/hostile-home"
hostile_stage="$tmp/hostile-stage"
hostile_marker="$tmp/ambient-smudge-executed"
hostile_attributes="$tmp/ambient-attributes"
hostile_smudge="$tmp/ambient-smudge"
mkdir -p "$hostile_home" "$hostile_stage"
printf '* filter=redline-hostile\n' >"$hostile_attributes"
printf '#!/usr/bin/env bash\ntouch "$REDLINE_RUSTSEC_SMUDGE_MARKER"\ncat\n' >"$hostile_smudge"
chmod 0755 "$hostile_smudge"
git config --file "$hostile_home/.gitconfig" core.attributesFile "$hostile_attributes"
git config --file "$hostile_home/.gitconfig" filter.redline-hostile.required true
git config --file "$hostile_home/.gitconfig" filter.redline-hostile.smudge "$hostile_smudge"
(
    export HOME="$hostile_home"
    export REDLINE_RUSTSEC_SMUDGE_MARKER="$hostile_marker"
    export JAIN_RUSTSEC_ADVISORY_SOURCE="$db"
    redline_stage_local_rustsec "$hostile_stage"
)
[[ ! -e "$hostile_marker" ]] || {
    printf 'ambient Git attributes/smudge executed during sealed RustSec staging\n' >&2
    exit 1
}

ln -s -- "$db" "$tmp/linked-db"
expect_rejected symlink env JAIN_PINNED_ADVISORY_DB="$tmp/linked-db" \
    JAIN_ADVISORY_DB="$tmp/linked-db" JAIN_PINNED_ADVISORY_COMMIT="$REDLINE_RUSTSEC_COMMIT" \
    bash -c 'source "$1"; redline_resolve_pinned_rustsec' bash \
    "$repo_root/ops/ci/pinned-rustsec.sh"
printf 'dirty\n' >"$db/untracked"
expect_rejected dirty env JAIN_PINNED_ADVISORY_DB="$db" JAIN_ADVISORY_DB="$db" \
    JAIN_PINNED_ADVISORY_COMMIT="$REDLINE_RUSTSEC_COMMIT" \
    bash -c 'source "$1"; redline_resolve_pinned_rustsec' bash \
    "$repo_root/ops/ci/pinned-rustsec.sh"

printf 'pinned RustSec hostiles passed: release variables custody path commit tree cleanliness ambient-git-isolation\n'
