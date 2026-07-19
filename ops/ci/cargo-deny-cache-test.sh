#!/usr/bin/env bash
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

jain_ci_scratch_create "$ROOT_DIR" redline-web-cargo-deny-cache \
  || fail "unable to create a custody-safe in-repository scratch directory"
tmp="$JAIN_CI_SCRATCH_PATH"
cleanup() {
  local rc=$?
  trap - EXIT
  jain_ci_scratch_remove || exit 1
  exit "$rc"
}
trap cleanup EXIT

jain_ci_scratch_hostile_tests() {
  local hostile="$tmp/scratch-custody"
  local repo outside saved_parent saved_child sentinel
  mkdir -m 0700 -- "$hostile"

  repo="$hostile/symlinked-target"
  outside="$hostile/symlinked-target-outside"
  mkdir -m 0700 -- "$repo" "$outside"
  ln -s -- "$outside" "$repo/target"
  if (jain_ci_scratch_create "$repo" hostile); then
    fail "symlinked target was accepted before scratch creation"
  fi
  [[ ! -e "$outside/ci-tmp" ]] || fail "symlinked target received an out-of-root write"

  repo="$hostile/symlinked-parent"
  outside="$hostile/symlinked-parent-outside"
  mkdir -m 0700 -- "$repo" "$repo/target" "$outside"
  ln -s -- "$outside" "$repo/target/ci-tmp"
  if (jain_ci_scratch_create "$repo" hostile); then
    fail "symlinked scratch parent was accepted before scratch creation"
  fi
  [[ -z "$(find "$outside" -mindepth 1 -print -quit)" ]] \
    || fail "symlinked scratch parent received an out-of-root write"

  repo="$hostile/wrong-parent-mode"
  mkdir -m 0700 -- "$repo" "$repo/target" "$repo/target/ci-tmp"
  chmod 0777 -- "$repo/target/ci-tmp"
  if (jain_ci_scratch_create "$repo" hostile); then
    fail "world-writable scratch parent was accepted"
  fi
  [[ -z "$(find "$repo/target/ci-tmp" -mindepth 1 -print -quit)" ]] \
    || fail "wrong-mode scratch parent received a child write"

  repo="$hostile/parent-swap"
  mkdir -m 0700 -- "$repo"
  (
    jain_ci_scratch_create "$repo" hostile
    saved_parent="${JAIN_CI_SCRATCH_PARENT}.saved"
    mv -- "$JAIN_CI_SCRATCH_PARENT" "$saved_parent"
    outside="$repo/outside"
    mkdir -m 0700 -- "$outside" "$outside/$JAIN_CI_SCRATCH_LEAF"
    sentinel="$outside/$JAIN_CI_SCRATCH_LEAF/sentinel"
    printf 'preserve\n' >"$sentinel"
    ln -s -- ../outside "$JAIN_CI_SCRATCH_PARENT"
    ! jain_ci_scratch_remove
    [[ -f "$sentinel" ]]
  ) || fail "parent-swap cleanup did not fail closed"

  repo="$hostile/child-swap"
  mkdir -m 0700 -- "$repo"
  (
    jain_ci_scratch_create "$repo" hostile
    saved_child="${JAIN_CI_SCRATCH_PATH}.saved"
    mv -- "$JAIN_CI_SCRATCH_PATH" "$saved_child"
    mkdir -m 0700 -- "$JAIN_CI_SCRATCH_PATH"
    sentinel="$JAIN_CI_SCRATCH_PATH/sentinel"
    printf 'preserve\n' >"$sentinel"
    ! jain_ci_scratch_remove
    [[ -f "$sentinel" ]]
  ) || fail "child-swap cleanup did not fail closed"

  local find_mode='' find_calls=0 find_rc
  find() {
    command find "$@"
    find_rc=$?
    ((find_calls += 1))
    if (( find_calls == 1 )) && [[ "$find_mode" == replacement ]]; then
      saved_child="${JAIN_CI_SCRATCH_PATH}.saved"
      mv -- "$JAIN_CI_SCRATCH_PATH" "$saved_child"
      mkdir -m 0700 -- "$JAIN_CI_SCRATCH_PATH"
      sentinel="$JAIN_CI_SCRATCH_PATH/sentinel"
      printf 'preserve replacement\n' >"$sentinel"
    elif (( find_calls == 1 )) && [[ "$find_mode" == child-nlink ]]; then
      mkdir -m 0700 -- "/proc/self/fd/${JAIN_CI_SCRATCH_PATH_FD}/post-clean-drift"
    fi
    return "$find_rc"
  }

  repo="$hostile/former-rmdir-replacement"
  mkdir -m 0700 -- "$repo"
  (
    jain_ci_scratch_create "$repo" hostile
    printf 'remove through held descriptor\n' >"$JAIN_CI_SCRATCH_PATH/payload"
    find_mode=replacement
    find_calls=0
    ! jain_ci_scratch_remove
    [[ -f "$sentinel" && -d "$saved_child" ]]
    [[ -z "$(command find "$saved_child" -mindepth 1 -print -quit)" ]]
  ) || fail "former-rmdir replacement was deleted or original child was not retained"

  repo="$hostile/parent-nlink-drift"
  mkdir -m 0700 -- "$repo"
  (
    jain_ci_scratch_create "$repo" hostile
    sentinel="$JAIN_CI_SCRATCH_PATH/sentinel"
    printf 'preserve on parent nlink drift\n' >"$sentinel"
    mkdir -m 0700 -- "$JAIN_CI_SCRATCH_PARENT/drift"
    ! jain_ci_scratch_remove
    [[ -f "$sentinel" ]]
  ) || fail "parent link-count drift was not rejected before cleanup"

  repo="$hostile/child-nlink-drift"
  mkdir -m 0700 -- "$repo"
  (
    jain_ci_scratch_create "$repo" hostile
    printf 'remove before child drift\n' >"$JAIN_CI_SCRATCH_PATH/payload"
    find_mode=child-nlink
    find_calls=0
    ! jain_ci_scratch_remove
    [[ -d "/proc/self/fd/${JAIN_CI_SCRATCH_PATH_FD}/post-clean-drift" ]]
  ) || fail "post-clean child link-count drift was not rejected"
  unset -f find
}

jain_ci_scratch_hostile_tests

export GIT_CONFIG_NOSYSTEM=1
export GIT_CONFIG_GLOBAL=/dev/null
export HTTP_PROXY=http://127.0.0.1:9 HTTPS_PROXY=http://127.0.0.1:9
export ALL_PROXY=http://127.0.0.1:9 NO_PROXY=
export http_proxy="$HTTP_PROXY" https_proxy="$HTTPS_PROXY"
export all_proxy="$ALL_PROXY" no_proxy=

log_root="$tmp/log"
mkdir "$log_root"
passing_summary='advisories ok, bans ok, licenses ok, sources ok'
clean_log="$log_root/clean.log"
printf '%s\n' "$passing_summary" >"$clean_log"
jain_verify_cargo_deny_clean_log "$clean_log"

expect_log_rejected() {
  local label="$1" contents="$2"
  local log_file="$log_root/$label.log"
  printf '%s' "$contents" >"$log_file"
  if jain_verify_cargo_deny_clean_log "$log_file" \
    >"$log_root/$label.stdout" 2>"$log_root/$label.stderr"; then
    printf '%s cargo-deny log fixture was accepted\n' "$label" >&2
    exit 1
  fi
  grep -F 'cargo-deny log must be the exact LF-terminated passing summary' \
    "$log_root/$label.stderr" >/dev/null
}

expect_log_rejected missing-lf "$passing_summary"
expect_log_rejected prefix $'unexpected prefix\nadvisories ok, bans ok, licenses ok, sources ok\n'
expect_log_rejected suffix $'advisories ok, bans ok, licenses ok, sources ok\nunexpected suffix\n'
expect_log_rejected error $'[ERROR] cargo-deny diagnostic\nadvisories ok, bans ok, licenses ok, sources ok\n'
expect_log_rejected fetch $'failed to fetch crates\nadvisories ok, bans ok, licenses ok, sources ok\n'
expect_log_rejected download $'failed to download crate\nadvisories ok, bans ok, licenses ok, sources ok\n'
expect_log_rejected offline $'offline request attempted\nadvisories ok, bans ok, licenses ok, sources ok\n'
expect_log_rejected network $'network access attempted\nadvisories ok, bans ok, licenses ok, sources ok\n'
expect_log_rejected url $'https://crates.io/index\nadvisories ok, bans ok, licenses ok, sources ok\n'
expect_log_rejected warn $'warning: advisory fetch skipped\nadvisories ok, bans ok, licenses ok, sources ok\n'

case_root="$tmp/case"
cache_parent="$case_root/cache"
fixed_cache="$cache_parent/index.crates.io-1949cf8c6b5b557f"
cargo_home="$case_root/cargo-home"
lock_file="$case_root/Cargo.lock"

reset_fixture() {
  local valuable_checksum wincon_checksum
  rm -rf -- "$case_root"
  mkdir -p "$fixed_cache" "$cargo_home"
  printf 'valuable fixture archive\n' >"$fixed_cache/valuable-0.1.1.crate"
  printf 'anstyle-wincon fixture archive\n' >"$fixed_cache/anstyle-wincon-3.0.11.crate"
  valuable_checksum="$(jain_sha256 "$fixed_cache/valuable-0.1.1.crate")"
  wincon_checksum="$(jain_sha256 "$fixed_cache/anstyle-wincon-3.0.11.crate")"
  cat >"$lock_file" <<EOF
version = 3

[[package]]
name = "anstyle-wincon"
version = "3.0.11"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "$wincon_checksum"

[[package]]
name = "valuable"
version = "0.1.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "$valuable_checksum"
EOF
}

expect_rejected() {
  local label="$1" expected="$2"
  local stderr_file="$case_root/$label.stderr"
  if jain_seed_locked_cargo_archives \
    "$lock_file" "$cache_parent" "$fixed_cache" "$cargo_home" \
    valuable 0.1.1 anstyle-wincon 3.0.11 \
    >"$case_root/$label.stdout" 2>"$stderr_file"; then
    printf '%s fixture was accepted\n' "$label" >&2
    exit 1
  fi
  grep -F -- "$expected" "$stderr_file" >/dev/null || {
    printf '%s fixture failed without expected diagnostic: %s\n' "$label" "$expected" >&2
    cat "$stderr_file" >&2
    exit 1
  }
}

reset_fixture
[[ -z "$(find "$cargo_home" -mindepth 1 -print -quit)" ]]
expected_records="$(printf '%s\t%s\t%s\n' \
  anstyle-wincon 3.0.11 "$(jain_sha256 "$fixed_cache/anstyle-wincon-3.0.11.crate")" \
  valuable 0.1.1 "$(jain_sha256 "$fixed_cache/valuable-0.1.1.crate")")"
[[ "$(jain_locked_registry_package_records "$lock_file")" == "$expected_records" ]]
jain_seed_locked_cargo_archives \
  "$lock_file" "$cache_parent" "$fixed_cache" "$cargo_home" \
  valuable 0.1.1 anstyle-wincon 3.0.11
destination="$cargo_home/registry/cache/$(basename "$fixed_cache")"
[[ "$(jain_sha256 "$destination/valuable-0.1.1.crate")" \
  == "$(jain_sha256 "$fixed_cache/valuable-0.1.1.crate")" ]]
[[ "$(jain_sha256 "$destination/anstyle-wincon-3.0.11.crate")" \
  == "$(jain_sha256 "$fixed_cache/anstyle-wincon-3.0.11.crate")" ]]

reset_fixture
jain_seed_locked_cargo_registry_closure \
  "$lock_file" "$cache_parent" "$fixed_cache" "$cargo_home"
destination="$cargo_home/registry/cache/$(basename "$fixed_cache")"
[[ "$(find "$destination" -mindepth 1 -maxdepth 1 -type f | wc -l)" -eq 2 ]]
printf 'post-seed unlocked archive\n' >"$destination/unlocked-1.0.0.crate"
if jain_verify_locked_cargo_registry_closure \
  "$lock_file" "$fixed_cache" "$cargo_home" \
  >"$case_root/post-seed-unlocked.stdout" \
  2>"$case_root/post-seed-unlocked.stderr"; then
  printf 'post-seed unlocked destination fixture was accepted\n' >&2
  exit 1
fi
grep -F 'isolated Cargo archive destination contains missing or unlocked entries' \
  "$case_root/post-seed-unlocked.stderr" >/dev/null

reset_fixture
destination="$cargo_home/registry/cache/$(basename "$fixed_cache")"
mkdir -p "$destination"
printf 'not locked\n' >"$destination/unlocked-1.0.0.crate"
if jain_seed_locked_cargo_registry_closure \
  "$lock_file" "$cache_parent" "$fixed_cache" "$cargo_home" \
  >"$case_root/unlocked-destination.stdout" \
  2>"$case_root/unlocked-destination.stderr"; then
  printf 'unlocked destination fixture was accepted\n' >&2
  exit 1
fi
grep -F 'locked registry closure destination must not preexist' \
  "$case_root/unlocked-destination.stderr" >/dev/null

reset_fixture
rm -- "$fixed_cache/valuable-0.1.1.crate"
expect_rejected missing 'locked crate archive is missing'

reset_fixture
rm -- "$fixed_cache/anstyle-wincon-3.0.11.crate"
ln -s /dev/null "$fixed_cache/anstyle-wincon-3.0.11.crate"
expect_rejected symlink 'fixed host Cargo cache must be a flat physical file set'

reset_fixture
printf 'corruption\n' >>"$fixed_cache/valuable-0.1.1.crate"
expect_rejected wrong-digest 'locked crate archive digest does not match Cargo.lock'

reset_fixture
destination_cache="$cargo_home/registry/cache/$(basename "$fixed_cache")"
mkdir -p "$destination_cache"
ln -s /dev/null "$destination_cache/valuable-0.1.1.crate"
expect_rejected destination-symlink 'existing Cargo archive destination is not the locked artifact'

reset_fixture
duplicate_cache="$cache_parent/index.crates.io-duplicate"
mkdir "$duplicate_cache"
printf 'malicious out-of-root duplicate\n' >"$duplicate_cache/valuable-0.1.1.crate"
jain_seed_locked_cargo_archives \
  "$lock_file" "$cache_parent" "$fixed_cache" "$cargo_home" \
  valuable 0.1.1 anstyle-wincon 3.0.11
destination="$cargo_home/registry/cache/$(basename "$fixed_cache")"
[[ "$(jain_sha256 "$destination/valuable-0.1.1.crate")" \
  == "$(jain_sha256 "$fixed_cache/valuable-0.1.1.crate")" ]]
[[ "$(jain_sha256 "$destination/valuable-0.1.1.crate")" \
  != "$(jain_sha256 "$duplicate_cache/valuable-0.1.1.crate")" ]]

reset_fixture
mkdir -p "$fixed_cache/nested"
cp -- "$fixed_cache/valuable-0.1.1.crate" "$fixed_cache/nested/valuable-0.1.1.crate"
expect_rejected duplicate-within-root 'fixed host Cargo cache must be a flat physical file set'

advisory_root="$tmp/advisory"
source_db="$advisory_root/source-db"
mkdir -p "$source_db"
git -C "$source_db" init -q -b main
git -C "$source_db" config user.name 'Redline Web CI Fixture'
git -C "$source_db" config user.email 'redline-web-ci@example.invalid'
printf 'first advisory fixture\n' >"$source_db/README.md"
git -C "$source_db" add README.md
git -C "$source_db" commit -q -m 'first fixture'
first_commit="$(git -C "$source_db" rev-parse HEAD)"
printf 'second advisory fixture\n' >>"$source_db/README.md"
git -C "$source_db" add README.md
git -C "$source_db" commit -q -m 'second fixture'
advisory_commit="$(git -C "$source_db" rev-parse HEAD)"
advisory_tree="$(git -C "$source_db" rev-parse 'HEAD^{tree}')"
git -C "$source_db" remote add origin "$source_db"
git -C "$source_db" fetch -q --no-tags origin refs/heads/main

new_advisory_home() {
  advisory_home="$advisory_root/home-$1"
  rm -rf -- "$advisory_home"
  mkdir "$advisory_home"
}

expect_advisory_seed_rejected() {
  local label="$1" candidate_source="$2" commit="$3" tree="$4" expected="$5"
  local stderr_file="$advisory_root/$label.stderr"
  new_advisory_home "$label"
  if jain_seed_cargo_deny_advisory_db \
    "$candidate_source" "$advisory_home" "$commit" "$tree" \
    >"$advisory_root/$label.stdout" 2>"$stderr_file"; then
    printf '%s advisory DB fixture was accepted\n' "$label" >&2
    exit 1
  fi
  grep -F -- "$expected" "$stderr_file" >/dev/null || {
    printf '%s advisory DB fixture failed without expected diagnostic: %s\n' \
      "$label" "$expected" >&2
    cat "$stderr_file" >&2
    exit 1
  }
}

expect_advisory_verify_rejected() {
  local label="$1" repository="$2" expected="$3"
  local stderr_file="$advisory_root/$label.stderr"
  if jain_verify_isolated_cargo_deny_db \
    "$repository" "$advisory_commit" "$advisory_tree" \
    >"$advisory_root/$label.stdout" 2>"$stderr_file"; then
    printf '%s isolated advisory DB fixture was accepted\n' "$label" >&2
    exit 1
  fi
  grep -F -- "$expected" "$stderr_file" >/dev/null || {
    printf '%s isolated advisory DB fixture failed without expected diagnostic: %s\n' \
      "$label" "$expected" >&2
    cat "$stderr_file" >&2
    exit 1
  }
}

new_advisory_home success
jain_seed_cargo_deny_advisory_db \
  "$source_db" "$advisory_home" "$advisory_commit" "$advisory_tree"
isolated_db="$advisory_home/advisory-dbs/advisory-db-3157b0e258782691"
jain_verify_isolated_cargo_deny_db "$isolated_db" "$advisory_commit" "$advisory_tree"

expect_advisory_seed_rejected \
  missing-db "$advisory_root/missing" "$advisory_commit" "$advisory_tree" \
  'fixed cargo-deny advisory DB must be a physical checkout'
ln -s "$source_db" "$advisory_root/linked-db"
expect_advisory_seed_rejected \
  linked-db "$advisory_root/linked-db" "$advisory_commit" "$advisory_tree" \
  'fixed cargo-deny advisory DB must be a physical checkout'
expect_advisory_seed_rejected \
  wrong-head "$source_db" "$first_commit" "$advisory_tree" \
  'fixed cargo-deny advisory DB HEAD/tree/clean identity mismatch'
expect_advisory_seed_rejected \
  wrong-tree "$source_db" "$advisory_commit" \
  0000000000000000000000000000000000000000 \
  'fixed cargo-deny advisory DB HEAD/tree/clean identity mismatch'

confused_source="$advisory_root/advisory-db"
git clone -q --no-local --no-tags --single-branch --branch main \
  "$source_db" "$confused_source"
git -C "$confused_source" fetch -q --no-tags "$source_db" refs/heads/main
mkdir "$confused_source/advisory-db-3157b0e258782691"
printf 'host lock confusion fixture\n' >"$confused_source/db.lock"
expect_advisory_seed_rejected \
  singular-dirty-path "$confused_source" "$advisory_commit" "$advisory_tree" \
  'fixed cargo-deny advisory DB HEAD/tree/clean identity mismatch'

detached_source="$advisory_root/detached-source"
git clone -q --no-local --no-checkout "$source_db" "$detached_source"
git -C "$detached_source" checkout -q --detach "$advisory_commit"
new_advisory_home detached-source
jain_seed_cargo_deny_advisory_db \
  "$detached_source" "$advisory_home" "$advisory_commit" "$advisory_tree"
jain_verify_isolated_cargo_deny_db \
  "$advisory_home/advisory-dbs/advisory-db-3157b0e258782691" \
  "$advisory_commit" "$advisory_tree"

export JAIN_PINNED_ADVISORY_DB="$detached_source"
export JAIN_ADVISORY_DB=
[[ "$(jain_governed_advisory_db_path "$advisory_commit")" == "$detached_source" ]]
export JAIN_PINNED_ADVISORY_DB=
export JAIN_ADVISORY_DB="$detached_source"
[[ "$(jain_governed_advisory_db_path "$advisory_commit")" == "$detached_source" ]]
export JAIN_PINNED_ADVISORY_DB="$detached_source"
jain_verify_governed_advisory_db \
  "$detached_source" "$advisory_commit" "$advisory_tree"
if jain_verify_governed_advisory_db \
  "$advisory_root/missing" "$advisory_commit" "$advisory_tree" \
  >/dev/null 2>&1; then
  printf 'missing governed advisory database was accepted\n' >&2
  exit 1
fi
if jain_verify_governed_advisory_db \
  "$advisory_root/linked-db" "$advisory_commit" "$advisory_tree" \
  >/dev/null 2>&1; then
  printf 'symlinked governed advisory database was accepted\n' >&2
  exit 1
fi
if jain_verify_governed_advisory_db \
  "$detached_source" "$advisory_commit" \
  0000000000000000000000000000000000000000 >/dev/null 2>&1; then
  printf 'wrong-tree governed advisory database was accepted\n' >&2
  exit 1
fi

export JAIN_ADVISORY_DB="$source_db"
if jain_governed_advisory_db_path "$advisory_commit" >/dev/null 2>&1; then
  printf 'conflicting governed advisory paths were accepted\n' >&2
  exit 1
fi
export JAIN_ADVISORY_DB="$detached_source"
export JAIN_PINNED_ADVISORY_COMMIT="$first_commit"
if jain_governed_advisory_db_path "$advisory_commit" >/dev/null 2>&1; then
  printf 'wrong governed advisory commit was accepted\n' >&2
  exit 1
fi
unset JAIN_PINNED_ADVISORY_COMMIT

grep -Fq 'JAIN_PINNED_ADVISORY_DB' "$ROOT_DIR/ops/ci/lib.sh"
grep -Fq 'JAIN_ADVISORY_DB' "$ROOT_DIR/ops/ci/lib.sh"
if grep -q '9f3e138091487e69144f536d36976e427a7a3307\|c33f1047906505cabcec7e21f2d99db5c6de8852' \
  "$ROOT_DIR/ops/ci/lib.sh" "$ROOT_DIR/ops/ci/security.sh"; then
  printf 'obsolete RustSec commit or tree remains in the active CI surface\n' >&2
  exit 1
fi

new_advisory_home fetch-head
jain_seed_cargo_deny_advisory_db \
  "$source_db" "$advisory_home" "$advisory_commit" "$advisory_tree"
isolated_db="$advisory_home/advisory-dbs/advisory-db-3157b0e258782691"
git -C "$isolated_db" fetch -q --no-tags "$source_db" "$first_commit"
expect_advisory_verify_rejected fetch-head "$isolated_db" 'FETCH_HEAD identity mismatch'

new_advisory_home destination-link
jain_seed_cargo_deny_advisory_db \
  "$source_db" "$advisory_home" "$advisory_commit" "$advisory_tree"
isolated_db="$advisory_home/advisory-dbs/advisory-db-3157b0e258782691"
ln -s /dev/null "$isolated_db/linked"
expect_advisory_verify_rejected destination-link "$isolated_db" 'contains a symlink'

new_advisory_home alternates
jain_seed_cargo_deny_advisory_db \
  "$source_db" "$advisory_home" "$advisory_commit" "$advisory_tree"
isolated_db="$advisory_home/advisory-dbs/advisory-db-3157b0e258782691"
printf '%s\n' "$source_db/.git/objects" >"$isolated_db/.git/objects/info/alternates"
expect_advisory_verify_rejected alternates "$isolated_db" 'contains alternates or hooks'

new_advisory_home unexpected-ref
jain_seed_cargo_deny_advisory_db \
  "$source_db" "$advisory_home" "$advisory_commit" "$advisory_tree"
isolated_db="$advisory_home/advisory-dbs/advisory-db-3157b0e258782691"
git -C "$isolated_db" update-ref refs/heads/unexpected "$advisory_commit"
expect_advisory_verify_rejected unexpected-ref "$isolated_db" 'unexpected refs or remotes'

new_advisory_home nonclean
jain_seed_cargo_deny_advisory_db \
  "$source_db" "$advisory_home" "$advisory_commit" "$advisory_tree"
isolated_db="$advisory_home/advisory-dbs/advisory-db-3157b0e258782691"
printf 'dirty\n' >>"$isolated_db/README.md"
expect_advisory_verify_rejected nonclean "$isolated_db" 'HEAD/tree/clean identity mismatch'

printf 'cargo-deny locked archive and advisory DB hostile tests passed\n'
