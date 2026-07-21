#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mkdir -p "$repo_root/target/test-tmp"
tmp="$(mktemp -d "$repo_root/target/test-tmp/jain-host-ci-integrity-test.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
fixture="$tmp/control"
mkdir -p "$fixture/ops/ci" "$fixture/tools/splitctl/src"

extract_contract_parser() {
  local script="$1"
  sed -n \
    '/^jain_contract_source_object() {$/,/^}$/p' "$script"
}

parser_fixture="$tmp/contract-mirror"
valid_object="$(printf 'a%.0s' {1..40})"
for boundary in host-ci-sandbox.sh split-host-ci.sh; do
  parser="$(extract_contract_parser "$repo_root/ops/ci/$boundary")"
  [[ -n "$parser" ]] || {
    printf '%s omits the contract source parser\n' "$boundary" >&2
    exit 1
  }
  printf 'Source-commit: %s\n' "$valid_object" >"$parser_fixture"
  [[ "$(bash -c "$parser; jain_contract_source_object \"\$1\"" \
    _ "$parser_fixture")" == "$valid_object" ]] || {
    printf '%s rejected one exact contract source object\n' "$boundary" >&2
    exit 1
  }
  for hostile in \
    "Source-commit: malformed" \
    "Source-commit: $valid_object
Source-commit: malformed" \
    "Source-commit: $valid_object
Source-commit: $valid_object"; do
    printf '%s\n' "$hostile" >"$parser_fixture"
    if bash -c "$parser; jain_contract_source_object \"\$1\"" \
      _ "$parser_fixture" >/dev/null 2>&1; then
      printf '%s accepted malformed or ambiguous contract source declarations\n' \
        "$boundary" >&2
      exit 1
    fi
  done
done
[[ "$(extract_contract_parser "$repo_root/ops/ci/host-ci-sandbox.sh")" \
  == "$(extract_contract_parser "$repo_root/ops/ci/split-host-ci.sh")" ]] || {
  printf 'root and worker contract source parsers differ\n' >&2
  exit 1
}

cp -- "$repo_root/ops/ci/host-ci-integrity.sh" "$fixture/ops/ci/host-ci-integrity.sh"
for path in \
  Cargo.lock Cargo.toml repos.manifest.toml \
  ops/ci/host-ci-publisher.sh ops/ci/host-ci-sandbox.sh \
  ops/ci/host-ci-boundary-preflight.sh \
  ops/ci/host-ci-proof-evidence.sh \
  ops/ci/cargo-lock-closure.sh \
  ops/ci/native-build-tools.lock.json \
  ops/ci/native-runtime.sh ops/ci/pnpm-runtime.sh \
  ops/ci/pnpm-store.lock.json ops/ci/pinned-advisory.sh \
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
grep -F 'jain_validate_pnpm_store "$pnpm_authority"' \
  "$repo_root/ops/ci/host-ci-sandbox.sh" >/dev/null || {
  printf 'root sandbox does not validate sealed pnpm custody\n' >&2
  exit 1
}
grep -F 'NPM_CONFIG_OFFLINE=true' \
  "$repo_root/ops/ci/host-ci-sandbox.sh" >/dev/null || {
  printf 'root sandbox does not force pnpm offline mode\n' >&2
  exit 1
}
grep -F 'NPM_CONFIG_PREFER_SYMLINKED_EXECUTABLES=false' \
  "$repo_root/ops/ci/host-ci-sandbox.sh" >/dev/null || {
  printf 'root sandbox does not prohibit pnpm executable symlinks\n' >&2
  exit 1
}
grep -F 'git config --file "$sibling_git_config"' \
  "$repo_root/ops/ci/split-host-ci.sh" >/dev/null || {
  printf 'reviewed worker lacks exact-path sibling ownership trust\n' >&2
  exit 1
}
grep -F -- '--add safe.directory "$sib_path/.git"' \
  "$repo_root/ops/ci/split-host-ci.sh" >/dev/null || {
  printf 'reviewed worker lacks exact sibling Git-directory trust\n' >&2
  exit 1
}
if grep -Eq 'safe\.directory=(\*|"?\$SPLIT_ROOT"?)' \
  "$repo_root/ops/ci/split-host-ci.sh"; then
  printf 'reviewed worker contains broad sibling ownership trust\n' >&2
  exit 1
fi
for required_source_binding in \
  '--ref refs/heads/main --resolve-ref-head' \
  'BindReadOnlyPaths=$sibling_checkout:$family_root/$sibling' \
  'sibling_sources_sha256' \
  'jain.host-ci-sibling-sources/v1'; do
  grep -F -- "$required_source_binding" \
    "$repo_root/ops/ci/host-ci-sandbox.sh" >/dev/null || {
    printf 'root sandbox lacks protected sibling source binding: %s\n' \
      "$required_source_binding" >&2
    exit 1
  }
done
grep -F 'missing sealed sibling authority' \
  "$repo_root/ops/ci/split-host-ci.sh" >/dev/null || {
  printf 'reviewed worker does not fail closed on absent sibling authority\n' >&2
  exit 1
}
for sibling_lock_binding in \
  'sealed_sibling_repositories' \
  'jain_capture_sorted_nul "$sibling_cargo_lock_list"' \
  '--lock "$sibling_lock_checkout/$sibling_cargo_lock_path"'; do
  grep -F -- "$sibling_lock_binding" \
    "$repo_root/ops/ci/split-host-ci.sh" >/dev/null || {
    printf 'reviewed worker omits sealed sibling Cargo lock closure: %s\n' \
      "$sibling_lock_binding" >&2
    exit 1
  }
done
grep -F 'Cargo cache receipt is not bound to the exact product/sibling lock closure' \
  "$repo_root/ops/ci/host-ci-isolated-contract-test.sh" >/dev/null || {
  printf 'isolated worker does not verify the sibling Cargo lock closure\n' >&2
  exit 1
}
for closure_binding in \
  'jain_render_cargo_lock_source_closure' \
  'lock-source-closure.json' \
  'staged Cargo lock per-source closure differs from independent authority'; do
  grep -F -- "$closure_binding" \
    "$repo_root/ops/ci/split-host-ci.sh" \
    "$repo_root/ops/ci/host-ci-isolated-contract-test.sh" >/dev/null || {
    printf 'host CI omits independently verified per-source lock closure: %s\n' \
      "$closure_binding" >&2
    exit 1
  }
done
grep -F 'sibling protected main moved before publication' \
  "$repo_root/ops/ci/host-ci-publisher.sh" >/dev/null || {
  printf 'root publisher does not reject protected sibling ref drift\n' >&2
  exit 1
}
for boundary in host-ci-sandbox.sh split-host-ci.sh host-ci-publisher.sh; do
  grep -F 'sibling_sources_sha256' "$repo_root/ops/ci/$boundary" >/dev/null || {
    printf '%s does not carry the sibling source digest\n' "$boundary" >&2
    exit 1
  }
done
for required_tag_binding in \
  '--retain-declared-release-tag' \
  'product_release_tag_ref' \
  'product_release_tag_commit'; do
  grep -F -- "$required_tag_binding" \
    "$repo_root/ops/ci/host-ci-sandbox.sh" >/dev/null || {
    printf 'root sandbox lacks sealed product release-tag binding: %s\n' \
      "$required_tag_binding" >&2
    exit 1
  }
done
for boundary in host-ci-sandbox.sh split-host-ci.sh host-ci-publisher.sh; do
  grep -F 'product_release_tag_commit' \
    "$repo_root/ops/ci/$boundary" >/dev/null || {
    printf '%s does not carry the product release-tag identity\n' \
      "$boundary" >&2
    exit 1
  }
done
grep -F 'product release tag moved before success publication' \
  "$repo_root/ops/ci/host-ci-publisher.sh" >/dev/null || {
  printf 'root publisher does not reject product release-tag drift\n' >&2
  exit 1
}
ownership_fixture="$tmp/sibling-ownership"
git init --quiet "$ownership_fixture"
git -C "$ownership_fixture" config user.name 'Sibling Ownership Fixture'
git -C "$ownership_fixture" config user.email sibling@example.invalid
printf 'fixture\n' >"$ownership_fixture/input"
git -C "$ownership_fixture" add input
git -C "$ownership_fixture" commit --quiet -m exact
if GIT_TEST_ASSUME_DIFFERENT_OWNER=1 \
  git -C "$ownership_fixture" rev-parse --verify 'HEAD^{commit}' \
    >/dev/null 2>&1; then
  printf 'Git ownership regression fixture did not become dubious\n' >&2
  exit 1
fi
ownership_head="$(GIT_TEST_ASSUME_DIFFERENT_OWNER=1 \
  git -c safe.directory="$ownership_fixture" -C "$ownership_fixture" \
    rev-parse --verify 'HEAD^{commit}')" || exit 1
[[ "$ownership_head" == "$(git -C "$ownership_fixture" rev-parse HEAD)" ]] \
  || exit 1
ownership_config="$tmp/sibling-safe-directory.config"
git config --file "$ownership_config" \
  --add safe.directory "$ownership_fixture"
git config --file "$ownership_config" \
  --add safe.directory "$ownership_fixture/.git"
chmod 0600 "$ownership_config"
GIT_TEST_ASSUME_DIFFERENT_OWNER=1 GIT_CONFIG_NOSYSTEM=1 \
  GIT_CONFIG_GLOBAL="$ownership_config" \
  git clone --quiet --no-local --no-checkout \
    "$ownership_fixture" "$tmp/sibling-ownership-clone"
[[ "$(git -C "$tmp/sibling-ownership-clone" \
  rev-parse --verify "$ownership_head^{commit}")" == "$ownership_head" ]] \
  || exit 1
grep -F 'jain_validate_nvidia_smi_detector "$nvidia_smi_path"' \
  "$repo_root/ops/ci/host-ci-sandbox.sh" >/dev/null || {
  printf 'root sandbox does not validate the NVIDIA detector\n' >&2
  exit 1
}
grep -F 'jain_run_nvidia_smi_detector "$nvidia_smi_path"' \
  "$repo_root/ops/ci/host-ci-sandbox.sh" >/dev/null || {
  printf 'root sandbox does not derive CUDA capability itself\n' >&2
  exit 1
}
grep -F 'select(.device_allow == [])' \
  "$repo_root/ops/ci/host-ci-sandbox.sh" >/dev/null || {
  printf 'root sandbox permits GPU device allowance\n' >&2
  exit 1
}
grep -F 'caller-provided CUDA_COMPUTE_CAP is forbidden' \
  "$repo_root/ops/ci/split-host-ci-parent.sh" >/dev/null || {
  printf 'host-CI parent does not reject ambient CUDA capability\n' >&2
  exit 1
}
if sed -n '/safe_child_vars=(/,/^)/p' \
  "$repo_root/ops/ci/split-host-ci-parent.sh" | grep -Fq CUDA_COMPUTE_CAP; then
  printf 'host-CI parent serializes caller CUDA capability\n' >&2
  exit 1
fi
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
