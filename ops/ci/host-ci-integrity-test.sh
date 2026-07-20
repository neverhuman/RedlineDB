#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mkdir -p "$repo_root/target/test-tmp"
tmp="$(mktemp -d "$repo_root/target/test-tmp/jain-host-ci-integrity-test.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
fixture="$tmp/control"
mkdir -p "$fixture/ops/ci" "$fixture/tools/splitctl/src"

cp -- "$repo_root/ops/ci/host-ci-integrity.sh" "$fixture/ops/ci/host-ci-integrity.sh"
for path in \
  Cargo.lock Cargo.toml repos.manifest.toml \
  ops/ci/host-ci-publisher.sh ops/ci/host-ci-sandbox.sh \
  ops/ci/host-ci-boundary-preflight.sh \
  ops/ci/host-ci-proof-evidence.sh \
  ops/ci/native-build-tools.lock.json \
  ops/ci/native-runtime.sh ops/ci/pinned-advisory.sh \
  ops/ci/pinned-cargo-audit.sh ops/ci/pinned-cargo-deny.sh \
  ops/ci/split-host-ci-parent.sh ops/ci/split-host-ci.sh \
  tools/splitctl/src/jeryu_client.rs \
  tools/splitctl/src/main.rs; do
  mkdir -p "$fixture/$(dirname "$path")"
  printf 'fixture %s\n' "$path" >"$fixture/$path"
done
chmod +x "$fixture/ops/ci/host-ci-integrity.sh"
grep -F 'jain_validate_native_build_tools "$native_build_tools_authority"' \
  "$repo_root/ops/ci/host-ci-sandbox.sh" >/dev/null || {
  printf 'root sandbox does not validate native build-tool custody\n' >&2
  exit 1
}
grep -F 'BindReadOnlyPaths=$native_build_tools_root:$native_build_tools_mount' \
  "$repo_root/ops/ci/host-ci-sandbox.sh" >/dev/null || {
  printf 'root sandbox does not bind native build tools read-only\n' >&2
  exit 1
}
grep -F 'jain_activate_native_build_tools' \
  "$repo_root/ops/ci/split-host-ci.sh" >/dev/null || {
  printf 'reviewed worker does not activate validated native build tools\n' >&2
  exit 1
}
for boundary in host-ci-sandbox.sh host-ci-publisher.sh; do
  grep -F '"$splitctl_path" host-ci-authority' "$repo_root/ops/ci/$boundary" >/dev/null \
    || {
      printf '%s does not derive repository authority through splitctl\n' "$boundary" >&2
      exit 1
    }
  if grep -F 'awk -v wanted="$repo"' "$repo_root/ops/ci/$boundary" >/dev/null; then
    printf '%s still contains a second repository-authority parser\n' "$boundary" >&2
    exit 1
  fi
done

assert_guard_precedes_checkout() {
  local path="${1:?path is required}" guard="${2:?guard is required}"
  local checkout="${3:?checkout is required}" guard_line checkout_line
  guard_line="$(grep -nF "$guard" "$repo_root/$path" | head -n1 | cut -d: -f1)"
  checkout_line="$(grep -nF "$checkout" "$repo_root/$path" \
    | awk -F: -v guard="$guard_line" '$1 > guard { print $1; exit }')"
  [[ "$guard_line" =~ ^[0-9]+$ && "$checkout_line" =~ ^[0-9]+$ \
    && "$guard_line" -lt "$checkout_line" ]] || {
    printf '%s does not reject the object tree before checkout\n' "$path" >&2
    exit 1
  }
}
assert_guard_precedes_checkout ops/ci/host-ci-sandbox.sh \
  'jain_git_object_tree_is_symlink_free "${arguments[3]}"' \
  'checkout --quiet --detach "${arguments[2]}"'
assert_guard_precedes_checkout ops/ci/host-ci-sandbox.sh \
  'jain_git_object_tree_is_symlink_free "$audit_worktree"' \
  'checkout --quiet --detach "${arguments[2]}"'
assert_guard_precedes_checkout ops/ci/split-host-ci.sh \
  'jain_git_object_tree_is_symlink_free "$wt" "$SHA"' \
  'git -C "$wt" checkout --quiet --detach "$SHA"'
assert_guard_precedes_checkout ops/ci/split-host-ci.sh \
  'jain_git_object_tree_is_symlink_free "$tmp/$sib" "$sib_sha"' \
  'git -C "$tmp/$sib" checkout --quiet --detach "$sib_sha"'
git init --quiet "$fixture"
git -C "$fixture" config user.name 'Host CI Fixture'
git -C "$fixture" config user.email host-ci-fixture@example.invalid
git -C "$fixture" add .
git -C "$fixture" commit --quiet -m exact
fixture_commit="$(git -C "$fixture" rev-parse HEAD)"
remote="$tmp/control.git"
git init --quiet --bare "$remote"
git -C "$fixture" remote add origin "$remote"
git -C "$fixture" push --quiet -u origin HEAD:main
config_exec="$tmp/config-exec.sh"
config_marker="$tmp/config-exec.marker"
cat >"$config_exec" <<SCRIPT
#!/usr/bin/env bash
printf 'executed\n' >>'$config_marker'
exit 0
SCRIPT
chmod 0700 "$config_exec"
git -C "$fixture" config core.fsmonitor "$config_exec"
git -C "$fixture" config diff.external "$config_exec"
git -C "$fixture" config remote.origin.uploadpack "$config_exec"
[[ "$("$fixture/ops/ci/host-ci-integrity.sh" \
  "$fixture" "$fixture_commit")" == "$fixture_commit" ]] || exit 1
[[ ! -e "$config_marker" ]] || {
  printf 'host CI integrity executed repository-local config\n' >&2
  exit 1
}
git -C "$fixture" config --unset core.fsmonitor
git -C "$fixture" config --unset diff.external
git -C "$fixture" config --unset remote.origin.uploadpack
if "$fixture/ops/ci/host-ci-integrity.sh" "$fixture" \
  0123456789abcdef0123456789abcdef01234567 >/dev/null 2>&1; then
  printf 'host CI integrity accepted a different expected commit\n' >&2
  exit 1
fi

# Published-ref mode deliberately ignores the editable checkout's HEAD and
# working bytes. Root authenticates this returned commit against the configured
# forge ref before executing any control-plane code.
git -C "$fixture" switch --quiet -c feature/ahead
printf 'feature-only\n' >>"$fixture/ops/ci/native-runtime.sh"
git -C "$fixture" add ops/ci/native-runtime.sh
git -C "$fixture" commit --quiet -m feature
printf 'uncommitted operator bytes\n' >>"$fixture/ops/ci/native-runtime.sh"
[[ "$("$fixture/ops/ci/host-ci-integrity.sh" "$fixture" \
  --ref refs/remotes/origin/main)" == "$fixture_commit" ]] || {
  printf 'host CI did not resolve protected origin/main independently of checkout HEAD\n' >&2
  exit 1
}
if "$fixture/ops/ci/host-ci-integrity.sh" "$fixture" \
  --ref refs/remotes/origin/unpublished >/dev/null 2>&1; then
  printf 'host CI accepted an unpublished bootstrap ref\n' >&2
  exit 1
fi
git -C "$fixture" reset --quiet --hard
git -C "$fixture" switch --quiet main

printf 'dirty runtime\n' >>"$fixture/ops/ci/native-runtime.sh"
if "$fixture/ops/ci/host-ci-integrity.sh" "$fixture" >/dev/null 2>&1; then
  printf 'host CI integrity accepted a dirty native runtime\n' >&2
  exit 1
fi
git -C "$fixture" restore ops/ci/native-runtime.sh

printf 'dirty runner\n' >>"$fixture/ops/ci/split-host-ci.sh"
if "$fixture/ops/ci/host-ci-integrity.sh" "$fixture" >/dev/null 2>&1; then
  printf 'host CI integrity accepted a dirty status runner\n' >&2
  exit 1
fi
git -C "$fixture" restore ops/ci/split-host-ci.sh

printf 'dirty publisher\n' >>"$fixture/ops/ci/host-ci-publisher.sh"
if "$fixture/ops/ci/host-ci-integrity.sh" "$fixture" >/dev/null 2>&1; then
  printf 'host CI integrity accepted a dirty root publisher source\n' >&2
  exit 1
fi
git -C "$fixture" restore ops/ci/host-ci-publisher.sh

printf 'dirty transport\n' >>"$fixture/tools/splitctl/src/jeryu_client.rs"
if "$fixture/ops/ci/host-ci-integrity.sh" "$fixture" >/dev/null 2>&1; then
  printf 'host CI integrity accepted dirty Jeryu transport source\n' >&2
  exit 1
fi
git -C "$fixture" restore tools/splitctl/src/jeryu_client.rs

printf 'dirty proof evidence policy\n' >>"$fixture/ops/ci/host-ci-proof-evidence.sh"
if "$fixture/ops/ci/host-ci-integrity.sh" "$fixture" >/dev/null 2>&1; then
  printf 'host CI integrity accepted dirty proof evidence policy\n' >&2
  exit 1
fi
git -C "$fixture" restore ops/ci/host-ci-proof-evidence.sh

printf 'untracked orchestration\n' >"$fixture/ops/ci/unreviewed.sh"
if "$fixture/ops/ci/host-ci-integrity.sh" "$fixture" >/dev/null 2>&1; then
  printf 'host CI integrity accepted unreviewed control-plane bytes\n' >&2
  exit 1
fi
rm -- "$fixture/ops/ci/unreviewed.sh"

printf '# dirty integrity gate\n' >>"$fixture/ops/ci/host-ci-integrity.sh"
if "$fixture/ops/ci/host-ci-integrity.sh" "$fixture" >/dev/null 2>&1; then
  printf 'host CI integrity accepted its own dirty bytes\n' >&2
  exit 1
fi

printf 'host CI exact orchestration contract ok\n'
