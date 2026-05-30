#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$repo_root"

if command -v rtk >/dev/null 2>&1; then
    run() {
        rtk "$@"
    }
else
    run() {
        "$@"
    }
fi

# shellcheck source=ops/ci/lib.sh
. "$repo_root/ops/ci/lib.sh"

log() {
    printf 'ci-fast-push: %s\n' "$*" >&2
}

git_author_name="${GIT_AUTHOR_NAME:-redline-ci-bot}"
git_author_email="${GIT_AUTHOR_EMAIL:-41898282+github-actions[bot]@users.noreply.github.com}"
github_main_sha=""
sqlite_parity_repetitions="${REDLINEDB_SQLITE_PARITY_REPETITIONS:-3}"
sqlite_parity_warmup="${REDLINEDB_SQLITE_PARITY_WARMUP:-1}"

set_git_identity() {
    export GIT_AUTHOR_NAME="$git_author_name"
    export GIT_AUTHOR_EMAIL="$git_author_email"
    export GIT_COMMITTER_NAME="$git_author_name"
    export GIT_COMMITTER_EMAIL="$git_author_email"
}

commit_tree_if_needed() {
    local message="$1"

    git add -A
    if git diff --cached --quiet; then
        log "no tracked drift to commit"
        return 0
    fi

    set_git_identity
    git -c user.name="$git_author_name" -c user.email="$git_author_email" commit --no-verify -m "$message"
    log "committed $(git rev-parse --short HEAD)"
}

snapshot_tree() {
    commit_tree_if_needed "chore(ci): snapshot local tree for github/main [skip ci]"
}

commit_ci_drift() {
    commit_tree_if_needed "chore(ci): capture ci-generated drift [skip ci]"
}

enable_local_redline_testing() {
    if [ -n "${CI_REDLINE_TESTING_LOCAL_BIN:-}" ] || [ -n "${CI_REDLINE_TESTING_LOCAL_SOURCE:-}" ]; then
        return 0
    fi

    local default_bin="$HOME/redline-testing/target/release/redline-testing"
    local default_source="$HOME/redline-testing"
    if [ -x "$default_bin" ] && [ -d "$default_source" ]; then
        export CI_REDLINE_TESTING_LOCAL_BIN="$default_bin"
        export CI_REDLINE_TESTING_LOCAL_SOURCE="$default_source"
        log "using local redline-testing escape hatch: $default_bin"
    fi
}

redline_testing_tmp_root() {
    if [ -d /dev/shm ] && [ -w /dev/shm ]; then
        printf '%s\n' "/dev/shm/redline-testing"
    else
        printf '%s/redline-testing\n' "${TMPDIR:-/tmp}"
    fi
}

copy_redline_testing_provenance() {
    local redline_testing_bin="${1:?redline-testing bin required}"
    local destination="${2:?provenance destination required}"
    local provenance_source

    provenance_source="$(dirname "$(dirname "$redline_testing_bin")")/redline-testing-provenance.env"
    mkdir -p "$(dirname "$destination")"
    if [ -f "$provenance_source" ]; then
        cp "$provenance_source" "$destination"
    else
        log "redline-testing provenance sidecar missing: $provenance_source"
        return 1
    fi
}

fetch_github_main() {
    run git fetch github main --quiet
    github_main_sha="$(git rev-parse github/main)"
    log "fetched github/main at $github_main_sha"
}

ensure_head_contains_github_main() {
    if ! git merge-base --is-ancestor github/main HEAD; then
        log "HEAD does not contain github/main; rebase or merge the latest main and rerun"
        exit 1
    fi
}

ensure_tokei() {
    local cargo_bin_dir="${CARGO_HOME:-$HOME/.cargo}/bin"
    export PATH="$cargo_bin_dir:$PATH"

    if command -v tokei >/dev/null 2>&1; then
        return 0
    fi

    log "installing tokei for README metrics regeneration"
    run cargo install tokei --locked --version 14.0.0
}

run_step() {
    local label="$1"
    shift

    log "$label"
    if "$@"; then
        return 0
    else
        local rc=$?
        log "$label failed with exit $rc"
        return "$rc"
    fi
}

run_fast_surface() {
    local stage

    for stage in preflight core kernel sql-unit sql-contracts bench; do
        run_step "fast/$stage" run env CI_FAST_STAGE="$stage" bash ops/ci/fast.sh || return $?
    done
}

run_official_parity_with_cap() {
    local redline_testing_bin
    local sqlite_parity_reference_bin
    local rc=0

    export CI_REDLINE_TESTING_REQUESTED_VERSION="${CI_REDLINE_TESTING_REQUESTED_VERSION:-1.0.0}"
    export REDLINE_TESTING_PINNED_ONLY="${REDLINE_TESTING_PINNED_ONLY:-1}"
    export REDLINEDB_DEFAULT_DURABILITY=normal
    export REDLINEDB_QUIET_DURABILITY=1

    enable_local_redline_testing

    run_step "parity/build-redlinedb" run cargo build -p redlinedb-cli --release --bin redlinedb --locked || return $?
    if [ ! -x target/release/redlinedb ]; then
        log "expected release binary missing: target/release/redlinedb"
        return 1
    fi

    if [ -n "${REDLINEDB_SQLITE_PARITY_SQLITE_BIN:-}" ]; then
        sqlite_parity_reference_bin="$REDLINEDB_SQLITE_PARITY_SQLITE_BIN"
    else
        sqlite_parity_reference_bin="$(run bash scripts/sqlite/build-reference.sh)"
        export REDLINEDB_SQLITE_PARITY_SQLITE_BIN="$sqlite_parity_reference_bin"
    fi
    if [ ! -x "$sqlite_parity_reference_bin" ]; then
        log "expected SQLite reference binary missing: $sqlite_parity_reference_bin"
        return 1
    fi
    if [ "$(sha256sum target/release/redlinedb | awk '{print $1}')" = "$(sha256sum "$sqlite_parity_reference_bin" | awk '{print $1}')" ]; then
        log "redline-testing target and SQLite reference unexpectedly hash-identical: target/release/redlinedb"
        return 1
    fi

    redline_testing_bin="$(ci_install_redline_testing)"
    load_redline_testing_provenance "$redline_testing_bin"

    mkdir -p target/redline-testing
    copy_redline_testing_provenance "$redline_testing_bin" target/redline-testing/redline-testing-provenance.env

    run_step "parity/run-redline-testing" "$redline_testing_bin" run \
        --target-bin target/release/redlinedb \
        --sqlite-bin "$sqlite_parity_reference_bin" \
        --suite all \
        --workers 40 \
        --tmp-root "$(redline_testing_tmp_root)" \
        --repetitions "$sqlite_parity_repetitions" \
        --warmup "$sqlite_parity_warmup" \
        --output target/redline-testing/all.jsonl || rc=$?

    if [ "$rc" -ne 0 ]; then
        if ! run_step "parity/tolerate-known-optional" run bash scripts/parity-tolerate-known-optional.sh target/redline-testing; then
            return "$rc"
        fi
    fi

    ci_assert_redline_testing_official_artifacts
    run_step "parity/process-official-evidence" run bash scripts/process-redline-testing-evidence.sh target/redline-testing || return $?
    ci_assert_artifact target/redline-testing/official-evidence.processed.json
}

run_ci_surface() {
    run_fast_surface || return $?
    run_official_parity_with_cap || return $?
    run_step "official-evidence-guard" run bash scripts/guard-official-evidence.sh || return $?
    run_step "security" run bash ops/ci/security.sh || return $?
    run_step "dependency-review" run bash ops/ci/dependency-review.sh || return $?
    run_step "jankurai-audit" run bash ops/ci/jankurai-audit.sh || return $?
    run_step "jankurai-tools" run bash scripts/ci-local.sh jankurai-tools || return $?
    ensure_tokei
    run_step "metrics-readme" run bash scripts/generate-metrics-readme.sh || return $?
}

main() {
    if [ "$#" -ne 0 ]; then
        log "usage: ./ci-fast-push.sh"
        exit 64
    fi

    snapshot_tree
    export REDLINEDB_BENCH_GIT_SHA="$(git rev-parse HEAD)"

    fetch_github_main
    ensure_head_contains_github_main

    check_status=0
    run_ci_surface || check_status=$?

    commit_ci_drift

    if [ "$check_status" -ne 0 ]; then
        exit "$check_status"
    fi

    run_step "staged-gate" env BASE_REF=github/main LOG_DIR=.jankurai/staged-gate-local bash ops/ci/jankurai-staged-gate.sh || exit $?

    fetch_github_main
    ensure_head_contains_github_main

    log "pushing HEAD $(git rev-parse --short HEAD) to github/main"
    run git push --force-with-lease=main:"$github_main_sha" github HEAD:main
}

main "$@"
