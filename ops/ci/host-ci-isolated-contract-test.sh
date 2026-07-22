#!/usr/bin/env bash
# Fast, hostile assertions that are meaningful only inside the real host-CI
# worker. Root-only broker installation and fault injection remain in
# split-host-ci-integrity-test.sh for ordinary exact-head review.
set -euo pipefail

fail() {
  printf 'isolated host-CI contract failed: %s\n' "$*" >&2
  exit 1
}

[[ "${JAIN_HOST_CI_NETWORK_ISOLATED:-0}" == 1 ]] \
  || fail 'worker isolation marker is absent'
[[ "${JAIN_HOST_CI_COMMAND_GIT_CONFIG_VALIDATED:-0}" == 1 ]] \
  || fail 'worker command Git configuration was not validated'

repo_root="$(git rev-parse --show-toplevel)"
writable_root="$(realpath -e -- "${JAIN_HOST_CI_WRITABLE_ROOT:?}")" \
  || fail 'bounded writable root is unavailable'
[[ -d "$writable_root" && ! -L "$writable_root" \
  && "$(stat -c '%u:%g' -- "$writable_root")" == "$(id -u):$(id -g)" ]] \
  || fail 'bounded writable root has the wrong identity'
cargo_home="$(realpath -e -- "${CARGO_HOME:?}")" \
  || fail 'fresh Cargo home is unavailable'
case "$cargo_home" in
  "$writable_root"/cargo-home) ;;
  *) fail 'Cargo home escaped the bounded request root' ;;
esac
[[ -d "$cargo_home" && ! -L "$cargo_home" \
  && "$(stat -c '%u:%g:%a' -- "$cargo_home")" == "$(id -u):$(id -g):700" \
  && "${CARGO_NET_OFFLINE:-}" == true \
  && "${CARGO_REGISTRIES_CRATES_IO_PROTOCOL:-}" == sparse \
  && -d /opt/jain-ci/cargo-registry \
  && ! -L /opt/jain-ci/cargo-registry \
  && -f "$cargo_home/registry/stage-receipt.json" \
    && ! -L "$cargo_home/registry/stage-receipt.json" ]] \
  || fail 'isolated Cargo cache is not the governed locked offline cache'
closure_helper=/opt/jain-ci/authority/control-plane/ops/ci/cargo-lock-closure.sh
[[ -f "$closure_helper" && ! -L "$closure_helper" ]] \
  || fail 'reviewed Cargo lock closure helper is unavailable'
# shellcheck source=ops/ci/cargo-lock-closure.sh
source "$closure_helper"
lock_list_root="$writable_root/isolated-cargo-lock-lists"
lock_source_records="$writable_root/isolated-cargo-lock-sources.jsonl"
mkdir -m 0700 "$lock_list_root" \
  || fail 'cannot create isolated Cargo lock list root'
: >"$lock_source_records" || fail 'cannot create isolated lock source records'
chmod 0600 "$lock_source_records"
product_head="$(git -C "$repo_root" rev-parse --verify 'HEAD^{commit}')" \
  || fail 'cannot resolve exact product lock source'
product_lock_list="$lock_list_root/product.locks"
jain_capture_sorted_nul "$product_lock_list" \
  /usr/bin/git -C "$repo_root" ls-files -z -- \
    Cargo.lock ':(glob)**/Cargo.lock' \
  || fail 'exact product Cargo lock enumeration failed'
jain_cargo_lock_source_record \
  "$(basename "$repo_root")" "$product_head" "$repo_root" "$product_lock_list" \
  >>"$lock_source_records" \
  || fail 'cannot bind exact product Cargo lock source'
if [[ "${JAIN_SIBLING_SOURCES_REQUIRED:-false}" == true ]]; then
  sibling_source_count="$(
    jq -er '.sources | length | select(. > 0)' "$JAIN_SIBLING_SOURCES_PATH"
  )" || fail 'cannot count sealed sibling lock sources'
  mapfile -t sibling_source_rows < <(
    jq -er '.sources[] | [.repository,.mount_path,.commit] | @tsv' \
      "$JAIN_SIBLING_SOURCES_PATH"
  )
  [[ "${#sibling_source_rows[@]}" == "$sibling_source_count" ]] \
    || fail 'cannot enumerate every sealed sibling lock source'
  for sibling_source_row in "${sibling_source_rows[@]}"; do
    IFS=$'\t' read -r sibling_repository sibling_mount_path \
      sibling_expected_commit <<<"$sibling_source_row"
    [[ "$sibling_mount_path" == "$JAIN_SPLIT_ROOT/$sibling_repository" \
      && -d "$sibling_mount_path/.git" && ! -L "$sibling_mount_path" \
      && ! -L "$sibling_mount_path/.git" ]] \
      || fail 'sealed sibling lock authority is not an exact checkout'
    sibling_actual_commit="$(/usr/bin/git \
      -c safe.directory="$sibling_mount_path" -C "$sibling_mount_path" \
      rev-parse --verify 'HEAD^{commit}')" \
      || fail 'cannot resolve sealed sibling lock source commit'
    [[ "$sibling_actual_commit" == "$sibling_expected_commit" ]] \
      || fail 'sealed sibling lock source commit differs from authority'
    sibling_lock_list="$lock_list_root/$sibling_repository.locks"
    jain_capture_sorted_nul "$sibling_lock_list" \
      /usr/bin/git -c safe.directory="$sibling_mount_path" \
        -C "$sibling_mount_path" ls-files -z -- \
        Cargo.lock ':(glob)**/Cargo.lock' \
      || fail 'sealed sibling Cargo lock enumeration failed'
    jain_cargo_lock_source_record \
      "$sibling_repository" "$sibling_actual_commit" \
      "$sibling_mount_path" "$sibling_lock_list" \
      >>"$lock_source_records" \
      || fail 'cannot bind sealed sibling Cargo lock source'
  done
fi
expected_lock_closure="$writable_root/expected-lock-source-closure.json"
jain_render_cargo_lock_source_closure \
  "$lock_source_records" "$expected_lock_closure" \
  || fail 'cannot render independently enumerated Cargo lock closure'
actual_lock_closure="$cargo_home/registry/lock-source-closure.json"
[[ -f "$actual_lock_closure" && ! -L "$actual_lock_closure" \
  && "$(stat -c '%u:%g:%a:%h' -- "$actual_lock_closure")" \
    == "$(id -u):$(id -g):600:1" \
  && -z "$(cmp -s -- "$expected_lock_closure" "$actual_lock_closure" \
    || printf different)" ]] \
  || fail 'staged Cargo lock per-source closure differs from independent authority'
jq -e --slurpfile closure "$expected_lock_closure" '
  select(.schema_version == "jain.locked-cargo-cache/v2")
  | select($closure | length == 1)
  | select(.lock_count == $closure[0].lock_count)
  | select(.lock_sha256s == $closure[0].lock_sha256s)
  | select(.package_count > 0)' \
  "$cargo_home/registry/stage-receipt.json" >/dev/null \
  || fail 'Cargo cache receipt is not bound to the exact product/sibling lock closure'
# A feature-branch control-plane check is driven once by the previously
# installed protected-main broker. Its v2 receipt predates this additive field.
# After this runner is installed, split-host-ci.sh requires the field before it
# executes product code, and this branch verifies the resulting effective trust.
if jq -e 'has("governed_git_repositories")' \
  "$cargo_home/registry/stage-receipt.json" >/dev/null; then
  jq -e '
    select((.governed_git_repositories | type) == "array")
    | select(.governed_git_repositories
        == (.governed_git_repositories | sort | unique))' \
    "$cargo_home/registry/stage-receipt.json" >/dev/null \
    || fail 'Cargo cache receipt has an invalid governed Git trust set'
  git_config_global="$(realpath -e -- "${GIT_CONFIG_GLOBAL:?}")" \
    || fail 'scoped release Git configuration is unavailable'
  [[ -f "$git_config_global" && ! -L "$git_config_global" \
    && "$(stat -c '%u:%g:%a' -- "$git_config_global")" == "$(id -u):$(id -g):600" \
    && "$git_config_global" == "$writable_root/ci-gitconfig" \
    && "${GIT_CONFIG_NOSYSTEM:-}" == 1 \
    && ! -v GIT_CONFIG_PARAMETERS && ! -v GIT_CONFIG_COUNT \
    && ! -v GIT_CONFIG_SYSTEM \
    && "$(git config --global --get core.fsmonitor)" == false \
    && "$(git config --global --get core.hooksPath)" == /dev/null ]] \
    || fail 'release Git configuration escaped the bounded writable root'
  [[ -z "$(compgen -A variable GIT_CONFIG_KEY_)" \
    && -z "$(compgen -A variable GIT_CONFIG_VALUE_)" ]] \
    || fail 'inherited command-scope Git configuration survived setup'
  mapfile -t expected_safe_directories < <(
    printf '%s\n' /opt/jain-ci/authority/control-plane \
      "$JAIN_PINNED_ADVISORY_DB" \
      "$JAIN_CARGO_DENY_ADVISORY_DB"
    jq -r --arg mirror_root "$JAIN_SPLIT_ROOT/target/bare-mirrors" \
      '.governed_git_repositories[] | "\($mirror_root)/\(.).git"' \
      "$cargo_home/registry/stage-receipt.json"
  )
  mapfile -t actual_safe_directories < <(
    git config --global --get-all safe.directory || true
  )
  [[ "${#actual_safe_directories[@]}" -eq "${#expected_safe_directories[@]}" ]] \
    || fail 'release Git trust set differs from the exact locked repositories'
  for safe_index in "${!expected_safe_directories[@]}"; do
    expected_safe_directory="${expected_safe_directories[$safe_index]}"
    [[ "${actual_safe_directories[$safe_index]}" == "$expected_safe_directory" \
      && "$expected_safe_directory" != '*' \
      && -d "$expected_safe_directory" && ! -L "$expected_safe_directory" \
      && "$(realpath -e -- "$expected_safe_directory")" == "$expected_safe_directory" ]] \
      || fail 'release Git trust is not an exact physical governed mirror path'
  done
fi
[[ -z "$(find "$cargo_home/registry" -xdev -type l -print -quit)" \
  && -z "$(find "$cargo_home/registry" -xdev ! -type d ! -type f -print -quit)" ]] \
  || fail 'staged Cargo registry contains a symlink or special node'
if touch /opt/jain-ci/cargo-registry/.worker-write-probe 2>/dev/null; then
  fail 'worker mutated the root Cargo registry cache'
fi

no_new_privs="$(awk '/^NoNewPrivs:/ {print $2}' /proc/self/status)"
[[ "$no_new_privs" == 1 ]] || fail 'NoNewPrivs is not enforced'
for capability in CapInh CapPrm CapEff CapBnd CapAmb; do
  value="$(awk -v name="$capability" '$1 == name ":" {print $2}' /proc/self/status)"
  [[ "$value" == 0000000000000000 ]] || fail "$capability is not empty"
done

if /usr/bin/sudo -n /usr/bin/true >/dev/null 2>&1; then
  fail 'worker reached sudo'
fi
# systemd masks InaccessiblePaths with a mode-000 placeholder inode. Its name
# can still be statted, but the worker must not be able to list or traverse it,
# and no broker child may resolve through it.
if [[ -r /usr/local/libexec/jain || -x /usr/local/libexec/jain ]]; then
  fail 'root broker directory is accessible'
fi
if [[ -e /usr/local/libexec/jain/jeryu-merge-token ]]; then
  fail 'root publisher credential is visible'
fi
if touch /opt/jain-ci/authority/.worker-write-probe 2>/dev/null; then
  fail 'worker mutated root authority'
fi
[[ -r /opt/jain-ci/authority/control-plane/repos.manifest.toml \
  && -x /opt/jain-ci/authority/splitctl ]] \
  || fail 'reviewed read-only authority is unavailable'

if /usr/bin/timeout 2 /usr/bin/curl -fsS \
    http://127.0.0.1:8787/api/v1/version >/dev/null 2>&1; then
  fail 'worker reached the host forge network'
fi

# Mock-heavy tests need a real TCP loopback inside the private namespace. This
# probe uses only Rust's standard library and never opens a host-facing socket.
loopback_source="$writable_root/loopback-probe.rs"
loopback_binary="$writable_root/loopback-probe"
printf '%s\n' \
  'use std::{io::{Read, Write}, net::{TcpListener, TcpStream}, thread};' \
  'fn main() {' \
  '  let listener = TcpListener::bind("127.0.0.1:0").unwrap();' \
  '  let address = listener.local_addr().unwrap();' \
  '  let server = thread::spawn(move || {' \
  '    let (mut stream, _) = listener.accept().unwrap();' \
  '    let mut byte = [0_u8; 1];' \
  '    stream.read_exact(&mut byte).unwrap();' \
  '    stream.write_all(&byte).unwrap();' \
  '  });' \
  '  let mut client = TcpStream::connect(address).unwrap();' \
  '  client.write_all(b"x").unwrap();' \
  '  let mut echoed = [0_u8; 1];' \
  '  client.read_exact(&mut echoed).unwrap();' \
  '  assert_eq!(&echoed, b"x");' \
  '  server.join().unwrap();' \
  '}' >"$loopback_source"
rustc --edition=2021 "$loopback_source" -o "$loopback_binary"
"$loopback_binary" || fail 'private loopback TCP mocks are unavailable'
rm -f -- "$loopback_source" "$loopback_binary"

head="$(git -C "$repo_root" rev-parse --verify 'HEAD^{commit}')"
[[ "$head" =~ ^[0-9a-f]{40}$ \
  && -z "$(git -C "$repo_root" status --porcelain=v1 --untracked-files=all)" ]] \
  || fail 'product checkout is not a clean exact commit'

probe="$writable_root/boundary-probe.txt"
printf 'boundary_no_new_privs=%s\n' "$no_new_privs" >"$probe"
printf 'boundary_capabilities=empty\n' >>"$probe"
printf 'boundary_root_authority=read-only\n' >>"$probe"
printf 'boundary_sudo=blocked\n' >>"$probe"
printf 'boundary_forge_network=blocked\n' >>"$probe"
printf 'boundary_private_loopback=available\n' >>"$probe"
printf 'boundary_exact_head=%s\n' "$head" >>"$probe"
chmod 0600 "$probe"

printf 'isolated host-CI hostile boundary contract ok\n'
