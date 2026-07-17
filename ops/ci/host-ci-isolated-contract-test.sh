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

repo_root="$(git rev-parse --show-toplevel)"
writable_root="$(realpath -e -- "${JAIN_HOST_CI_WRITABLE_ROOT:?}")" \
  || fail 'bounded writable root is unavailable'
[[ -d "$writable_root" && ! -L "$writable_root" \
  && "$(stat -c '%u:%g' -- "$writable_root")" == "$(id -u):$(id -g)" ]] \
  || fail 'bounded writable root has the wrong identity'
cargo_home="$(realpath -e -- "${CARGO_HOME:?}")" \
  || fail 'fresh Cargo home is unavailable'
case "$cargo_home" in
  "$writable_root"/physical-checkouts/split-host-ci.??????/cargo-home) ;;
  *) fail 'Cargo home escaped the bounded physical checkout' ;;
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
jq -e --arg lock_sha "$(sha256sum Cargo.lock | cut -d' ' -f1)" '
  select(.schema_version == "jain.locked-cargo-cache/v1")
  | select(.lock_sha256 == $lock_sha)
  | select(.package_count > 0)' \
  "$cargo_home/registry/stage-receipt.json" >/dev/null \
  || fail 'Cargo cache receipt is not bound to the exact lock'
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
printf 'boundary_exact_head=%s\n' "$head" >>"$probe"
chmod 0600 "$probe"

printf 'isolated host-CI hostile boundary contract ok\n'
