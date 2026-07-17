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

no_new_privs="$(awk '/^NoNewPrivs:/ {print $2}' /proc/self/status)"
[[ "$no_new_privs" == 1 ]] || fail 'NoNewPrivs is not enforced'
for capability in CapInh CapPrm CapEff CapBnd CapAmb; do
  value="$(awk -v name="$capability" '$1 == name ":" {print $2}' /proc/self/status)"
  [[ "$value" == 0000000000000000 ]] || fail "$capability is not empty"
done

if /usr/bin/sudo -n /usr/bin/true >/dev/null 2>&1; then
  fail 'worker reached sudo'
fi
if [[ -e /usr/local/libexec/jain ]]; then
  fail 'root broker directory is visible'
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
