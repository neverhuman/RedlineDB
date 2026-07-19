#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
bounded_git_setup="$(sed -n \
  '/^# Cross-repo dependency resolution/,/^# cargo-cache-stage/p' \
  "$repo_root/ops/ci/split-host-ci.sh")"
[[ "$bounded_git_setup" == *'ci_gitconfig='* ]] || {
  printf 'host CI bounded Git configuration setup is absent\n' >&2
  exit 1
}
if [[ "$bounded_git_setup" == *'$REPO'* ]]; then
  printf 'host CI bounded Git configuration is repository-conditional\n' >&2
  exit 1
fi
mkdir -p "$repo_root/target/test-tmp"
tmp="$(mktemp -d "$repo_root/target/test-tmp/jain-split-host-integrity-test.XXXXXX")"
# The worker traverses to separately bind-mounted fixture roots beneath this
# directory. Allow traversal without granting directory listing or file access.
chmod 0711 "$tmp"
native_evidence_root="$(mktemp -d "$repo_root/target/test-tmp/jain-native-evidence-test.XXXXXX")"
proof_evidence_root="$(mktemp -d "$repo_root/target/test-tmp/jain-proof-evidence-test.XXXXXX")"
forged_root=""
fd_attack_root=""
runner_pid=""
forge_pid=""
attack_pid=""
publisher_root="$tmp/root-publisher"
request_root="$tmp/root-requests"
worker_cache="$tmp/worker-cache"
cargo_registry_cache=""
grype_db_root=""
noexec_request_root=""
product_forge_root="$tmp/product-forge"
cleanup() {
  cleanup_rc=$?
  if [[ "$cleanup_rc" != 0 ]]; then
    for diagnostic in "$tmp"/*.log "$tmp"/*.stderr; do
      [[ -f "$diagnostic" ]] || continue
      printf 'diagnostic: %s\n' "$diagnostic" >&2
      tail -80 "$diagnostic" >&2 || true
    done
  fi
  if [[ -n "$runner_pid" ]] && kill -0 "$runner_pid" 2>/dev/null; then
    kill "$runner_pid" 2>/dev/null || true
    wait "$runner_pid" 2>/dev/null || true
  fi
  if [[ -n "$forge_pid" ]] && kill -0 "$forge_pid" 2>/dev/null; then
    kill "$forge_pid" 2>/dev/null || true
    wait "$forge_pid" 2>/dev/null || true
  fi
  if [[ -n "$attack_pid" ]] && kill -0 "$attack_pid" 2>/dev/null; then
    kill "$attack_pid" 2>/dev/null || true
    wait "$attack_pid" 2>/dev/null || true
  fi
  [[ -z "$forged_root" || "$forged_root" != "$tmp"/* ]] || rm -rf -- "$forged_root"
  [[ -z "$fd_attack_root" || "$fd_attack_root" != "$tmp"/* ]] || rm -rf -- "$fd_attack_root"
  sudo -n rm -rf -- "$publisher_root" 2>/dev/null || true
  sudo -n rm -rf -- "$request_root" 2>/dev/null || true
  sudo -n rm -rf -- "$worker_cache" 2>/dev/null || true
  case "$cargo_registry_cache" in
    /var/lib/jain-host-ci/integrity-cargo-registry.??????)
      sudo -n rm -rf -- "$cargo_registry_cache" 2>/dev/null || true
      ;;
    '') ;;
    *) printf 'refusing unsafe Cargo registry fixture cleanup: %s\n' \
         "$cargo_registry_cache" >&2 ;;
  esac
  case "$grype_db_root" in
    /var/lib/jain-host-ci/integrity-grype-db.??????)
      sudo -n rm -rf -- "$grype_db_root" 2>/dev/null || true
      ;;
    '') ;;
    *) printf 'refusing unsafe Grype database fixture cleanup: %s\n' \
         "$grype_db_root" >&2 ;;
  esac
  if [[ -n "$noexec_request_root" ]]; then
    sudo -n /usr/bin/umount -- "$noexec_request_root" 2>/dev/null || true
    sudo -n rmdir -- "$noexec_request_root" 2>/dev/null || true
  fi
  sudo -n rm -rf -- "$native_evidence_root" 2>/dev/null || true
  sudo -n rm -rf -- "$proof_evidence_root" 2>/dev/null || true
  sudo -n rm -rf -- "$control_remote" 2>/dev/null || true
  sudo -n rm -rf -- "$product_forge_root" 2>/dev/null || true
  rm -rf -- "$tmp"
  return "$cleanup_rc"
}
trap cleanup EXIT

control="$tmp/control"
control_remote="$tmp/jain-split-ops.git"
split_root="$tmp/split"
sandbox_family_root="$tmp/sandbox-family"
pinned_advisory_commit="$(sed -n \
  's/^JAIN_PINNED_RUSTSEC_COMMIT="\([0-9a-f]\{40\}\)"$/\1/p' \
  "$repo_root/ops/ci/pinned-advisory.sh")"
[[ "$pinned_advisory_commit" =~ ^[0-9a-f]{40}$ ]]
mkdir -p "$sandbox_family_root/target" "$sandbox_family_root/jain-core" \
  "$sandbox_family_root/jain-redline"
mkdir -p "$sandbox_family_root/target/bare-mirrors"
git init --quiet --bare \
  "$sandbox_family_root/target/bare-mirrors/jain-core.git"
redline_fixture_sources="$tmp/redline-fixture-sources"
mkdir -p "$redline_fixture_sources"
git init --quiet --initial-branch=main "$redline_fixture_sources/control"
git -C "$redline_fixture_sources/control" config user.name 'Redline Fixture'
git -C "$redline_fixture_sources/control" config user.email \
  redline-fixture@example.invalid
cat >"$redline_fixture_sources/control/repos.manifest.toml" <<EOF
family = "redline-split"
lock = "redline.lock.toml"
[control_plane]
name = "redline-split-ops"
path = "."
remote = "http://127.0.0.1:8787/git/jeryu/redline-split-ops.git"
required_check = "redline-split-ops/required"
[[repo]]
name = "redline-core"
path = "../redline-core"
remote = "http://127.0.0.1:8787/git/jeryu/redline-core.git"
required_check = "redline-core/required"
default_branch = "main"
current_tag = "redline-core-v4.1.0-jain.1"
EOF
printf '[proof]\ncutover_eligible = true\n' \
  >"$redline_fixture_sources/control/redline.lock.toml"
git -C "$redline_fixture_sources/control" add repos.manifest.toml redline.lock.toml
git -C "$redline_fixture_sources/control" commit --quiet -m 'synthetic canonical control'
git clone --quiet --no-local "$redline_fixture_sources/control" \
  "$sandbox_family_root/jain-redline/redline-split-ops"
git -C "$sandbox_family_root/jain-redline/redline-split-ops" remote remove origin
git init --quiet --initial-branch=main "$redline_fixture_sources/core"
git -C "$redline_fixture_sources/core" config user.name 'Redline Fixture'
git -C "$redline_fixture_sources/core" config user.email \
  redline-fixture@example.invalid
printf 'synthetic core\n' >"$redline_fixture_sources/core/payload.txt"
git -C "$redline_fixture_sources/core" add payload.txt
git -C "$redline_fixture_sources/core" commit --quiet -m 'synthetic canonical core'
git clone --quiet --no-local "$redline_fixture_sources/core" \
  "$sandbox_family_root/jain-redline/redline-core"
git -C "$sandbox_family_root/jain-redline/redline-core" remote remove origin
git clone --quiet --no-local "$redline_fixture_sources/core" \
  "$sandbox_family_root/jain-redline/redline-central"
git -C "$sandbox_family_root/jain-redline/redline-central" remote remove origin
git clone --quiet --no-local --no-checkout "$HOME/.cargo/advisory-db" \
  "$sandbox_family_root/target/advisory-db"
git -C "$sandbox_family_root/target/advisory-db" checkout --quiet --detach \
  "$pinned_advisory_commit"
git -C "$sandbox_family_root/target/advisory-db" remote remove origin
[[ -d "$sandbox_family_root/target/advisory-db/.git" ]]
product="$split_root/jain-report"
product_remote="$product_forge_root/veox/jain-report.git"
forge_log="$tmp/forge.log"
forge_state="$tmp/forge-state.jsonl"
forge_behavior="$tmp/forge-behavior"
forge_address_file="$tmp/forge-address"
started="$tmp/started"
continue_file="$tmp/continue"
attack_log="$tmp/candidate-attack.log"
survivor_log="$tmp/candidate-survivor.log"
runner_log="$tmp/runner.log"

# A real loopback HTTP server replaces the old PATH-injected curl shim. The
# publisher reaches it from the host network namespace; the candidate's actual
# forge attempt must be blocked by its isolated network namespace.
rustc --edition=2021 "$repo_root/ops/ci/fake-forge.rs" -o "$tmp/fake-forge"
"$tmp/fake-forge" "$forge_address_file" "$forge_log" \
  "$forge_state" "$forge_behavior" >"$tmp/forge.stderr" 2>&1 &
forge_pid=$!
for _ in $(seq 1 500); do
  [[ -s "$forge_address_file" ]] && break
  kill -0 "$forge_pid" 2>/dev/null || break
  sleep 0.01
done
[[ -s "$forge_address_file" ]] || {
  cat "$tmp/forge.stderr" >&2
  printf 'fake forge did not start\n' >&2
  exit 1
}
forge_base="$(tr -d '\n' <"$forge_address_file")"
forge_port="${forge_base##*:}"
touch "$forge_log"

# A deterministic stand-in exercises the installed-auditor boundary without
# trusting a user-owned product tool. Production still pins the governed
# Jankurai 1.6.11 binary and digest; only this isolated fixture substitutes its
# reviewed digest before committing the control-plane fixture.
fake_jankurai="$tmp/jankurai"
cat >"$fake_jankurai" <<'JANKURAI'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == --version ]]; then
  printf 'jankurai 1.6.11\n'
  exit 0
fi
[[ "${1:-}" == audit && "${2:-}" == . ]] || exit 64
shift 2
report=''
markdown=''
repairs=''
full=0
advisory=0
no_history=0
policy=''
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --full) full=1; shift ;;
    --mode) [[ "${2:-}" == advisory ]] || exit 65; advisory=1; shift 2 ;;
    --policy) policy="${2:-}"; shift 2 ;;
    --json) report="${2:-}"; shift 2 ;;
    --md) markdown="${2:-}"; shift 2 ;;
    --repair-queue-jsonl) repairs="${2:-}"; shift 2 ;;
    --no-score-history) no_history=1; shift ;;
    *) exit 66 ;;
  esac
done
[[ "$full" == 1 && "$advisory" == 1 && "$no_history" == 1 \
  && "$policy" == agent/audit-policy.toml \
  && "$report" == /opt/jain-ci/output/report.json \
  && "$markdown" == /opt/jain-ci/output/report.md \
  && "$repairs" == /opt/jain-ci/output/repair-queue.jsonl ]] || exit 67
source_read_only=true
if /usr/bin/touch .jankurai-boundary-write 2>/dev/null; then
  source_read_only=false
  rm -f .jankurai-boundary-write
fi
network_isolated=true
if /usr/bin/curl -fsS --max-time 1 '__FORGE_BASE__/health' >/dev/null 2>&1; then
  network_isolated=false
fi
head="$(git -c core.fsmonitor=false -c core.hooksPath=/dev/null \
  -c diff.external= rev-parse --short=7 HEAD)"
mode="$(cat agent/test-auditor-mode 2>/dev/null || printf valid)"
if [[ "$mode" == wrong-head ]]; then
  head="$(printf '0%.0s' {1..40})"
fi
score=92
decision_passed=true
if [[ "$mode" == score-failure ]]; then
  score=84
  decision_passed=false
fi
policy_sha="$(sha256sum agent/audit-policy.toml | cut -d' ' -f1)"
jq -n --arg head "$head" --arg policy_sha "$policy_sha" \
  --arg source_read_only "$source_read_only" \
  --arg network_isolated "$network_isolated" \
  --argjson score "$score" --argjson decision_passed "$decision_passed" \
  '{score:$score,repo:".",auditor_version:"1.6.11",
    input_fingerprint:("sha256:" + ("1" * 64)),
    policy_fingerprint:("sha256:" + $policy_sha),dirty_worktree:false,
    git:{head:$head,dirty_worktree:false},
    decision:{passed:$decision_passed,minimum_score:85,hard_findings:[],
      ratchet:{passed:true,baseline_score:90,allowed_drop:0}},
    caps_applied:[],conformance_decision:"pass",conformance_blockers:[],
    run_id:("fixture-audit-" + $head),
    policy:{path:"agent/audit-policy.toml",minimum_score:85,
      auditor_version:"1.6.11"},
    fixture:{source_read_only:($source_read_only == "true"),
      network_isolated:($network_isolated == "true")}}' >"$report"
printf '# fixture Jankurai report\n' >"$markdown"
printf '{"repair":"none"}\n' >"$repairs"
JANKURAI
sed -i "s#__FORGE_BASE__#$forge_base#g" "$fake_jankurai"
chmod 0755 "$fake_jankurai"
fake_jankurai_digest="$(sha256sum "$fake_jankurai" | cut -d' ' -f1)"

git clone --quiet --no-local "$repo_root" "$control"
git -C "$control" config user.name 'Host CI Integration Fixture'
git -C "$control" config user.email host-ci-integration@example.invalid
install -m 0644 "$repo_root/repos.manifest.toml" "$control/repos.manifest.toml"
install -D -m 0644 "$repo_root/authority/source-paths.txt" \
  "$control/authority/source-paths.txt"
for boundary_file in \
  ops/ci/host-ci-integrity.sh ops/ci/host-ci-publisher.sh \
  ops/ci/host-ci-sandbox.sh ops/ci/host-ci-boundary-preflight.sh \
  ops/ci/native-runtime.sh ops/ci/host-ci-evidence.sh \
  ops/ci/host-ci-proof-evidence.sh \
  ops/ci/pinned-advisory.sh \
  ops/ci/split-host-ci-parent.sh ops/ci/split-host-ci.sh; do
  install -D -m 0755 "$repo_root/$boundary_file" "$control/$boundary_file"
done
# The adversarial matrix deliberately creates more than the production
# retention window before its final inode-tamper variants. Keep those fixture
# receipts long enough to exercise the original sealed success authority.
sed -i \
  's/JAIN_PROOF_EVIDENCE_RETAIN_PER_CHECK=8/JAIN_PROOF_EVIDENCE_RETAIN_PER_CHECK=32/' \
  "$control/ops/ci/host-ci-proof-evidence.sh"
install -D -m 0644 "$repo_root/tools/splitctl/src/main.rs" \
  "$control/tools/splitctl/src/main.rs"
install -D -m 0644 "$repo_root/tools/splitctl/src/jeryu_client.rs" \
  "$control/tools/splitctl/src/jeryu_client.rs"
git init --quiet --bare "$control_remote"
sed -i \
  "s#/home/ubuntu/jain-split#$sandbox_family_root#g" \
  "$control/repos.manifest.toml"
sed -i \
  "s#remote = \"http://127.0.0.1:8787/git/veox/jain-split-ops.git\"#remote = \"$control_remote\"#" \
  "$control/repos.manifest.toml"
# The production controller is intentionally fixed to the live local forge.
# This standalone fixture compiles and validates against its own ephemeral
# loopback server so required CI never contends with or mutates that service.
sed -i \
  "s#http://127.0.0.1:8787#$forge_base#g" \
  "$control/repos.manifest.toml" \
  "$control/tools/splitctl/src/main.rs" \
  "$sandbox_family_root/jain-redline/redline-split-ops/repos.manifest.toml"
sed -i \
  "s#8787#$forge_port#g" \
  "$control/tools/splitctl/src/jeryu_client.rs"
git -C "$control" add authority/source-paths.txt repos.manifest.toml ops/ci tools/splitctl/src
git -C "$control" commit --quiet -m 'fixture reviewed host-CI boundary'
git -C "$control" switch -C main --quiet
git -C "$control" remote set-url origin "$control_remote"
git -C "$control" push --quiet -u origin main
control_commit="$(git -C "$control" rev-parse HEAD)"
bootstrap_control_ref=refs/heads/codex/host-ci-bootstrap-test
git -C "$control" push --quiet origin \
  "$control_commit:$bootstrap_control_ref"
git -C "$control" switch --quiet -c codex/host-ci-bootstrap-test \
  --track origin/codex/host-ci-bootstrap-test
export JAIN_HOST_CI_BOOTSTRAP_REF="$bootstrap_control_ref"
bootstrap_expires_at="$(( $(date +%s) + 3600 ))"
sudo -n chown -R root:root "$control_remote"

# Install the reviewed broker and its credential exactly as production does:
# executable root-only broker, adjacent root-only config. The server itself
# never receives the token through argv or environment.
publisher="$publisher_root/host-ci-publisher"
publisher_config="$publisher_root/host-ci-publisher.config.json"
sandbox="$publisher_root/host-ci-sandbox"
sandbox_config="$publisher_root/host-ci-sandbox.config.json"
splitctl="$publisher_root/splitctl"
jankurai="$publisher_root/jankurai"
security_tool_digest="$(sha256sum /usr/bin/true | cut -d' ' -f1)"
git_lfs_path=/usr/bin/git-lfs
git_lfs_digest="$(sha256sum "$git_lfs_path" | cut -d' ' -f1)"
sudo -n install -d -o root -g root -m 0711 "$publisher_root"
sudo -n install -d -o root -g root -m 0700 "$request_root"
mkdir -p "$(dirname "$product_remote")"
sudo -n install -o root -g root -m 0500 \
  "$control/ops/ci/host-ci-publisher.sh" "$publisher"
sudo -n install -o root -g root -m 0500 \
  "$control/ops/ci/host-ci-sandbox.sh" "$sandbox"
sudo -n install -d -o xbwork -g xbwork -m 0700 "$worker_cache"
sudo -n install -d -o root -g root -m 0755 /var/lib/jain-host-ci
cargo_registry_cache="$(
  sudo -n mktemp -d /var/lib/jain-host-ci/integrity-cargo-registry.XXXXXX
)"
[[ "$cargo_registry_cache" \
  == /var/lib/jain-host-ci/integrity-cargo-registry.?????? ]]
sudo -n install -d -o root -g root -m 0555 \
  "$cargo_registry_cache" \
  "$cargo_registry_cache/cache" \
  "$cargo_registry_cache/cache/index.crates.io-1949cf8c6b5b557f" \
  "$cargo_registry_cache/index" \
  "$cargo_registry_cache/index/index.crates.io-6f17d22bba15001f" \
  "$cargo_registry_cache/index/index.crates.io-6f17d22bba15001f/.cache"
sudo -n install -o root -g root -m 0444 /dev/null \
  "$cargo_registry_cache/index/index.crates.io-6f17d22bba15001f/config.json"
grype_db_root="$(
  sudo -n mktemp -d /var/lib/jain-host-ci/integrity-grype-db.XXXXXX
)"
[[ "$grype_db_root" == /var/lib/jain-host-ci/integrity-grype-db.?????? ]]
sudo -n install -d -o root -g root -m 0555 "$grype_db_root/6"
sudo -n install -o root -g root -m 0444 \
  /usr/bin/true "$grype_db_root/6/import.json"
sudo -n install -o root -g root -m 0444 \
  /usr/bin/true "$grype_db_root/6/vulnerability.db"
sudo -n chmod 0555 "$grype_db_root"
grype_db_inventory_sha256="$({
  for relative in 6/import.json 6/vulnerability.db; do
    printf '%s\t%s\t%s\n' "$relative" \
      "$(sudo -n stat -c %s -- "$grype_db_root/$relative")" \
      "$(sudo -n sha256sum -- "$grype_db_root/$relative" | cut -d' ' -f1)"
  done
} | sha256sum | cut -d' ' -f1)"
sudo -n chown root:root "$native_evidence_root"
sudo -n chmod 0700 "$native_evidence_root"
sudo -n chown root:root "$proof_evidence_root"
sudo -n chmod 0700 "$proof_evidence_root"
cargo build --locked --quiet --manifest-path "$control/Cargo.toml" \
  --bin splitctl --target-dir "$tmp/direct-control-target"
splitctl_digest="$(sha256sum "$tmp/direct-control-target/debug/splitctl" \
  | cut -d' ' -f1)"
sudo -n install -o root -g root -m 0500 \
  "$tmp/direct-control-target/debug/splitctl" "$splitctl"
sudo -n install -o root -g root -m 0555 "$fake_jankurai" "$jankurai"
for tool in actionlint grype syft; do
  sudo -n install -o root -g root -m 0555 \
    /usr/bin/true "$publisher_root/security-$tool"
done
publisher_token="$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')"
publisher_token_file="$publisher_root/jeryu-merge-token"
printf '%s' "$publisher_token" | sudo -n tee "$publisher_token_file" >/dev/null
sudo -n chown root:root "$publisher_token_file"
sudo -n chmod 0600 "$publisher_token_file"
jq -cn --arg digest "$(sha256sum "$control/ops/ci/host-ci-publisher.sh" | cut -d' ' -f1)" \
  --arg sandbox_digest "$(sha256sum "$control/ops/ci/host-ci-sandbox.sh" | cut -d' ' -f1)" \
  --arg splitctl_digest "$splitctl_digest" --arg jankurai_digest "$fake_jankurai_digest" \
  --arg security_tool_digest "$security_tool_digest" \
  --arg token_file "$publisher_token_file" \
  --arg git_base "$product_forge_root" \
  --arg remote "$control_remote" --arg requests "$request_root" \
  --arg control_ref "$bootstrap_control_ref" \
  --arg bootstrap_commit "$control_commit" \
  --arg bootstrap_expires_at "$bootstrap_expires_at" \
  --arg native_evidence_root "$native_evidence_root" \
  --arg proof_evidence_root "$proof_evidence_root" \
  --arg grype_db_root "$grype_db_root" \
  --arg grype_db_inventory_sha256 "$grype_db_inventory_sha256" \
  '{schema_version:"jain.host-ci-publisher-config/v5",
    publisher_sha256:$digest,sandbox_sha256:$sandbox_digest,
    splitctl_sha256:$splitctl_digest,jankurai_sha256:$jankurai_digest,
    security_tool_sha256:{actionlint:$security_tool_digest,
      grype:$security_tool_digest,syft:$security_tool_digest},
    grype_db_root:$grype_db_root,
    grype_db_inventory_sha256:$grype_db_inventory_sha256,
    forge_git_base:$git_base,
    control_remote:$remote,control_ref:$control_ref,
    bootstrap_commit:$bootstrap_commit,
    bootstrap_expires_at:$bootstrap_expires_at,request_root:$requests,
    native_evidence_root:$native_evidence_root,
    proof_evidence_root:$proof_evidence_root,
    max_seal_age_seconds:300,token_file:$token_file}' \
  | sudo -n tee "$publisher_config" >/dev/null
sudo -n chown root:root "$publisher_config"
sudo -n chmod 0600 "$publisher_config"
jq -cn --arg digest "$(sha256sum "$control/ops/ci/host-ci-sandbox.sh" | cut -d' ' -f1)" \
  --arg publisher_digest "$(sha256sum "$control/ops/ci/host-ci-publisher.sh" | cut -d' ' -f1)" \
  --arg splitctl_digest "$splitctl_digest" --arg jankurai_digest "$fake_jankurai_digest" \
  --arg security_tool_digest "$security_tool_digest" \
  --arg family "$sandbox_family_root" --arg cache "$worker_cache" \
  --arg cargo_registry_cache "$cargo_registry_cache" \
  --arg cargo_bin "$HOME/.cargo/bin" --arg rustup "$HOME/.rustup" \
  --arg git_lfs_path "$git_lfs_path" --arg git_lfs_digest "$git_lfs_digest" \
  --arg token_file "$publisher_token_file" \
  --arg git_base "$product_forge_root" \
  --arg remote "$control_remote" --arg requests "$request_root" \
  --arg control_ref "$bootstrap_control_ref" \
  --arg bootstrap_commit "$control_commit" \
  --arg bootstrap_expires_at "$bootstrap_expires_at" \
  --arg native_evidence_root "$native_evidence_root" \
  --arg proof_evidence_root "$proof_evidence_root" \
  --arg grype_db_root "$grype_db_root" \
  --arg grype_db_inventory_sha256 "$grype_db_inventory_sha256" \
  --argjson parent_uid "$(id -u)" --argjson parent_gid "$(id -g)" \
  '{schema_version:"jain.host-ci-sandbox-config/v6",
    sandbox_sha256:$digest,publisher_sha256:$publisher_digest,
    splitctl_sha256:$splitctl_digest,jankurai_sha256:$jankurai_digest,
    security_tool_sha256:{actionlint:$security_tool_digest,
      grype:$security_tool_digest,syft:$security_tool_digest},
    grype_db_root:$grype_db_root,
    grype_db_inventory_sha256:$grype_db_inventory_sha256,
    parent_uid:$parent_uid,parent_gid:$parent_gid,
    worker_user:"xbwork",worker_group:"xbwork",family_root:$family,
    worker_cache:$cache,cargo_registry_cache:$cargo_registry_cache,
    cargo_bin:$cargo_bin,rustup_home:$rustup,
    git_lfs_path:$git_lfs_path,git_lfs_sha256:$git_lfs_digest,
    control_remote:$remote,control_ref:$control_ref,
    bootstrap_commit:$bootstrap_commit,
    bootstrap_expires_at:$bootstrap_expires_at,forge_git_base:$git_base,
    token_file:$token_file,
    request_root:$requests,retain_requests:true,
    native_evidence_root:$native_evidence_root,
    proof_evidence_root:$proof_evidence_root,
    device_allow:[]}' | sudo -n tee "$sandbox_config" >/dev/null
sudo -n chown root:root "$sandbox_config"
sudo -n chmod 0600 "$sandbox_config"

# Root-installed security tools are part of the sealed broker authority. A
# replaced binary must fail before the caller request or forge can be touched.
sudo -n install -o root -g root -m 0555 \
  /usr/bin/false "$publisher_root/security-actionlint"
if sudo -n "$sandbox" "$tmp/nonexistent-security-tool-request" \
  >"$tmp/security-tool-tamper.log" 2>&1; then
  printf 'sandbox accepted a replaced security tool\n' >&2
  exit 1
fi
grep -Fq 'installed security tool digest/metadata mismatch: actionlint' \
  "$tmp/security-tool-tamper.log"
sudo -n install -o root -g root -m 0555 \
  /usr/bin/true "$publisher_root/security-actionlint"

sudo -n install -o root -g root -m 0555 /usr/bin/false "$jankurai"
if sudo -n "$sandbox" "$tmp/nonexistent-jankurai-request" \
  >"$tmp/jankurai-tamper.log" 2>&1; then
  printf 'sandbox accepted a replaced Jankurai binary\n' >&2
  exit 1
fi
grep -Fq 'installed Jankurai digest/version mismatch' \
  "$tmp/jankurai-tamper.log"
sudo -n install -o root -g root -m 0555 "$fake_jankurai" "$jankurai"

sudo -n cat "$sandbox_config" >"$tmp/git-lfs-valid-config.json"
sudo -n jq '.git_lfs_sha256 = "0000000000000000000000000000000000000000000000000000000000000000"' \
  "$sandbox_config" >"$tmp/git-lfs-tamper-config.json"
sudo -n install -o root -g root -m 0600 \
  "$tmp/git-lfs-tamper-config.json" "$sandbox_config"
if sudo -n "$sandbox" "$tmp/nonexistent-git-lfs-request" \
  >"$tmp/git-lfs-tamper.log" 2>&1; then
  printf 'sandbox accepted an unpinned git-lfs executable\n' >&2
  exit 1
fi
grep -Fq 'git-lfs executable digest, metadata, or version mismatch' \
  "$tmp/git-lfs-tamper.log"
sudo -n install -o root -g root -m 0600 \
  "$tmp/git-lfs-valid-config.json" "$sandbox_config"

sudo -n install -o root -g root -m 0444 \
  /usr/bin/false "$grype_db_root/6/vulnerability.db"
if sudo -n "$sandbox" "$tmp/nonexistent-grype-db-request" \
  >"$tmp/grype-db-tamper.log" 2>&1; then
  printf 'sandbox accepted a mutated Grype database\n' >&2
  exit 1
fi
grep -Fq 'Grype database inventory digest mismatch' \
  "$tmp/grype-db-tamper.log"
sudo -n install -o root -g root -m 0444 \
  /usr/bin/true "$grype_db_root/6/vulnerability.db"

sudo -n install -d -o root -g root -m 0555 "$grype_db_root/extra"
if sudo -n "$sandbox" "$tmp/nonexistent-grype-db-request" \
  >"$tmp/grype-db-extra-node.log" 2>&1; then
  printf 'sandbox accepted an extra Grype database node\n' >&2
  exit 1
fi
grep -Fq 'Grype database inventory is not closed' \
  "$tmp/grype-db-extra-node.log"
sudo -n rmdir -- "$grype_db_root/extra"

# The live host may mount /run with noexec. Root authority copied beneath such
# a request root cannot execute in the worker, so reject the mount before any
# request directory, service, proof, or forge publication exists.
noexec_request_root="$tmp/noexec-request-root"
sudo -n install -d -o root -g root -m 0700 "$noexec_request_root"
sudo -n /usr/bin/mount -t tmpfs \
  -o 'nodev,nosuid,noexec,size=1048576,nr_inodes=32,mode=0700' \
  jain-host-ci-noexec-test "$noexec_request_root"
sudo -n cat "$sandbox_config" >"$tmp/executable-sandbox-config.json"
sudo -n jq --arg request_root "$noexec_request_root" \
  '.request_root = $request_root' "$sandbox_config" \
  >"$tmp/noexec-sandbox-config.json"
sudo -n install -o root -g root -m 0600 \
  "$tmp/noexec-sandbox-config.json" "$sandbox_config"
noexec_log="$tmp/noexec-request-root.log"
if JAIN_HOST_CI_SANDBOX="$sandbox" JAIN_SPLIT_ROOT="$sandbox_family_root" \
  "$control/ops/ci/split-host-ci.sh" \
    veox jain-report "$control_commit" "$control" \
    jain-report/required >"$noexec_log" 2>&1; then
  printf 'sandbox accepted a noexec root request filesystem\n' >&2
  exit 1
fi
grep -Fq 'root request filesystem forbids worker execution' "$noexec_log"
sudo -n install -o root -g root -m 0600 \
  "$tmp/executable-sandbox-config.json" "$sandbox_config"
sudo -n /usr/bin/umount -- "$noexec_request_root"
sudo -n rmdir -- "$noexec_request_root"
noexec_request_root=""

# The installed root snapshotter must reject special or over-limit request
# inodes promptly. In particular, a FIFO cannot stall the sudo boundary.
snapshot_fifo="$tmp/snapshot-request.fifo"
mkfifo -m 0600 "$snapshot_fifo"
fifo_rc=0
/usr/bin/timeout 5 sudo -n "$splitctl" host-ci-snapshot-request \
  --source "$snapshot_fifo" --destination "$request_root/fifo.snapshot" \
  --expected-uid "$(id -u)" --expected-gid "$(id -g)" \
  --max-bytes 65536 >"$tmp/snapshot-fifo.log" 2>&1 || fifo_rc=$?
[[ "$fifo_rc" != 0 && "$fifo_rc" != 124 ]] || {
  printf 'installed request snapshotter accepted or hung on a FIFO\n' >&2
  exit 1
}
snapshot_oversized="$tmp/snapshot-request-oversized.json"
truncate -s 65537 "$snapshot_oversized"
chmod 0600 "$snapshot_oversized"
if sudo -n "$splitctl" host-ci-snapshot-request \
  --source "$snapshot_oversized" \
  --destination "$request_root/oversized.snapshot" \
  --expected-uid "$(id -u)" --expected-gid "$(id -g)" \
  --max-bytes 65536 >"$tmp/snapshot-oversized.log" 2>&1; then
  printf 'installed request snapshotter accepted a grown 64 KiB request\n' >&2
  exit 1
fi

mkdir -p "$product/scripts" "$product/agent" "$split_root/jain-core"
cat >"$product/agent/audit-policy.toml" <<'POLICY'
minimum_score = 85
allowed_score_drop = 0
required_tool = "jankurai"
required_tool_version = "1.6.11"
POLICY
cat >"$product/agent/jankurai-baseline.json" <<'BASELINE'
{"schema":"jain.split.jankurai-baseline/v1","score":90,"caps":[],"hard_findings":0,"auditor":"jankurai 1.6.11"}
BASELINE
printf 'valid\n' >"$product/agent/test-auditor-mode"
cat >"$product/scripts/ci-local.sh" <<'SCRIPT'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${JAIN_RELEASE_CI:-0}" == 1 ]]; then
  [[ "$(command -v jankurai)" \
    == /opt/jain-ci/authority/release-bin/jankurai ]]
  [[ "$(jankurai --version)" == 'jankurai 1.6.11' ]]
fi
if [[ "${JAIN_TEST_REQUIRE_ISOLATION:-0}" == 1 ]]; then
  : "${JAIN_TEST_ATTACK_URL:?}" "${JAIN_TEST_ROOT_CONFIG_PATH:?}"
  : "${JAIN_TEST_ROOT_REQUEST_PATH:?}"
  : "${JAIN_TEST_FS_MONITOR_PATH:?}"
  cat >"$JAIN_TEST_FS_MONITOR_PATH" <<'MONITOR'
#!/usr/bin/env bash
id -u >>"${0}.log"
exit 0
MONITOR
  chmod 0700 "$JAIN_TEST_FS_MONITOR_PATH"
  git -C "$PWD" config core.fsmonitor "$JAIN_TEST_FS_MONITOR_PATH"
  git -C "$PWD" config diff.external "$JAIN_TEST_FS_MONITOR_PATH"
  git -C "$PWD" config remote.origin.uploadpack "$JAIN_TEST_FS_MONITOR_PATH"
  git -C "$PWD" config core.sshCommand "$JAIN_TEST_FS_MONITOR_PATH"
  git -C "$PWD" status --porcelain >/dev/null 2>&1 || true
  immutable_control_write=0
  if printf '[core]\nfsmonitor = %s\n' "$JAIN_TEST_FS_MONITOR_PATH" \
    >>/opt/jain-ci/authority/control-plane/.git/config 2>/dev/null; then
    immutable_control_write=1
  fi
  no_new_privs="$(awk '/^NoNewPrivs:/ { print $2 }' /proc/self/status)"
  cap_inheritable="$(awk '/^CapInh:/ { print $2 }' /proc/self/status)"
  cap_permitted="$(awk '/^CapPrm:/ { print $2 }' /proc/self/status)"
  cap_effective="$(awk '/^CapEff:/ { print $2 }' /proc/self/status)"
  cap_bounding="$(awk '/^CapBnd:/ { print $2 }' /proc/self/status)"
  cap_ambient="$(awk '/^CapAmb:/ { print $2 }' /proc/self/status)"
  pid_namespace="$(readlink /proc/self/ns/pid)"
  pid_namespace_separate=0
  [[ "$pid_namespace" != "${JAIN_TEST_HOST_PID_NAMESPACE:?}" ]] \
    && pid_namespace_separate=1
  user_namespace="$(readlink /proc/self/ns/user)"
  user_namespace_separate=0
  [[ "$user_namespace" != "${JAIN_TEST_HOST_USER_NAMESPACE:?}" ]] \
    && user_namespace_separate=1
  rustsec_standalone=0
  if [[ "${JAIN_RUSTSEC_ADVISORY_SOURCE:-}" \
      == /opt/jain-ci/authority/advisory-db \
    && "${JAIN_PINNED_ADVISORY_DB:-}" \
      == /opt/jain-ci/authority/advisory-db \
    && "${JAIN_ADVISORY_DB:-}" \
      == /opt/jain-ci/authority/advisory-db \
    && -d "${JAIN_RUSTSEC_ADVISORY_SOURCE}/.git" \
    && ! -L "${JAIN_RUSTSEC_ADVISORY_SOURCE}/.git" ]]; then
    rustsec_standalone=1
  fi
  deny_db_physical=0
  if [[ "${JAIN_CARGO_DENY_ADVISORY_DB:-}" \
      == "$CARGO_HOME/advisory-dbs/advisory-db-3157b0e258782691" \
    && -d "$JAIN_CARGO_DENY_ADVISORY_DB/.git" \
    && ! -L "$JAIN_CARGO_DENY_ADVISORY_DB" \
    && ! -L "$JAIN_CARGO_DENY_ADVISORY_DB/.git" ]]; then
    deny_db_physical=1
  fi
  advisory_swap_blocked=0
  if ! mv -- "$CARGO_HOME/advisory-dbs" \
      "$CARGO_HOME/advisory-dbs.swap" 2>/dev/null; then
    advisory_swap_blocked=1
  else
    mv -- "$CARGO_HOME/advisory-dbs.swap" "$CARGO_HOME/advisory-dbs"
  fi
  bare_mirror_safe=0
  mapfile -t safe_directories < <(
    git config --global --get-all safe.directory 2>/dev/null || true
  )
  if [[ "${#safe_directories[@]}" == 0 ]]; then
    bare_mirror_safe=1
  fi
  evidence_staging_bounded=0
  if [[ "${JAIN_NATIVE_EVIDENCE_STAGING_ROOT:-}" \
      == "$JAIN_HOST_CI_WRITABLE_ROOT/native-evidence-staging" \
    && "$(stat -f -c '%T' -- "$JAIN_NATIVE_EVIDENCE_STAGING_ROOT")" == tmpfs \
    && "$(df -PB1 --output=size "$JAIN_NATIVE_EVIDENCE_STAGING_ROOT" \
      | tail -n 1 | tr -d ' ')" -le 33554432 ]]; then
    evidence_staging_bounded=1
  fi
  recovered=''
  visible_pids=0
  for process in /proc/[0-9]*; do
    [[ -d "$process" ]] || continue
    visible_pids=$((visible_pids + 1))
    for candidate in "$process/environ" "$process/cmdline" "$process"/fd/*; do
      [[ -r "$candidate" ]] || continue
      case "$candidate" in
        */fd/*) [[ -f "$candidate" ]] || continue ;;
      esac
      value="$(tr '\0' '\n' <"$candidate" 2>/dev/null \
        | sed -n 's/^JERYU_MERGE_TOKEN=//p; s/^Authorization: Bearer //p' \
        | head -1 || true)"
      if [[ -n "$value" ]]; then
        recovered="$value"
        break 2
      fi
    done
  done
  config_readable=0
  [[ -r "${JAIN_TEST_ROOT_CONFIG_PATH:?}" ]] && config_readable=1
  root_request_readable=0
  [[ -r "${JAIN_TEST_ROOT_REQUEST_PATH:?}" ]] && root_request_readable=1
  attack_credential="${recovered:-no-publication-credential-recovered}"
  forge_attempt=blocked
  if /usr/bin/curl -fsS --max-time 1 -X POST \
    "$JAIN_TEST_ATTACK_URL/candidate-forge" \
    -H "Authorization: Bearer $attack_credential" \
    -H 'content-type: application/json' \
    -d '{"candidate":"forged-success"}' >/dev/null 2>&1; then
    forge_attempt=connected
  fi
  sudo_attempt=blocked
  if /usr/bin/sudo -n /usr/bin/true >/dev/null 2>&1; then
    sudo_attempt=connected
  fi
  probe="$JAIN_HOST_CI_WRITABLE_ROOT/boundary-probe.txt"
  printf 'boundary_no_new_privs=%s\n' "$no_new_privs" >"$probe"
  printf 'boundary_cap_inheritable=%s\n' "$cap_inheritable" >>"$probe"
  printf 'boundary_cap_permitted=%s\n' "$cap_permitted" >>"$probe"
  printf 'boundary_cap_effective=%s\n' "$cap_effective" >>"$probe"
  printf 'boundary_cap_bounding=%s\n' "$cap_bounding" >>"$probe"
  printf 'boundary_cap_ambient=%s\n' "$cap_ambient" >>"$probe"
  printf 'boundary_immutable_control_write=%s\n' \
    "$immutable_control_write" >>"$probe"
  printf 'boundary_pid_namespace=%s\n' "$pid_namespace" >>"$probe"
  printf 'boundary_pid_namespace_separate=%s\n' \
    "$pid_namespace_separate" >>"$probe"
  printf 'boundary_user_namespace=%s\n' "$user_namespace" >>"$probe"
  printf 'boundary_user_namespace_separate=%s\n' \
    "$user_namespace_separate" >>"$probe"
  printf 'boundary_visible_pids=%s\n' "$visible_pids" >>"$probe"
  printf 'boundary_credential_recovered=%s\n' \
    "$([[ -n "$recovered" ]] && echo 1 || echo 0)" >>"$probe"
  printf 'boundary_root_config_readable=%s\n' "$config_readable" >>"$probe"
  printf 'boundary_root_request_readable=%s\n' \
    "$root_request_readable" >>"$probe"
  printf 'boundary_forge_attempt=%s\n' "$forge_attempt" >>"$probe"
  printf 'boundary_sudo_attempt=%s\n' "$sudo_attempt" >>"$probe"
  printf 'boundary_release_ci=%s\n' "${JAIN_RELEASE_CI:-missing}" >>"$probe"
  printf 'boundary_rustsec_standalone=%s\n' "$rustsec_standalone" >>"$probe"
  printf 'boundary_deny_db_physical=%s\n' "$deny_db_physical" >>"$probe"
  printf 'boundary_advisory_swap_blocked=%s\n' \
    "$advisory_swap_blocked" >>"$probe"
  printf 'boundary_bare_mirror_safe=%s\n' "$bare_mirror_safe" >>"$probe"
  printf 'boundary_evidence_staging_bounded=%s\n' \
    "$evidence_staging_bounded" >>"$probe"
  # A malicious background descendant must die with namespace PID 1 before
  # the publisher reads its credential.
  (
    while :; do
      sleep 1
    done
  ) >/dev/null 2>&1 &
  printf 'boundary_survivor_started=1\n' >>"$probe"
  printf 'boundary_probe_complete=1\n' >>"$probe"
  chmod 0600 "$probe"
fi
sleep "${JAIN_TEST_SLEEP_SECONDS:-0}"
[[ "${JAIN_TEST_FORCE_FAILURE:-0}" != 1 ]] || exit 9
SCRIPT
chmod +x "$product/scripts/ci-local.sh"
git init --quiet "$product"
git -C "$product" config user.name 'Product Fixture'
git -C "$product" config user.email product-fixture@example.invalid
git -C "$product" add .
git -C "$product" commit --quiet -m fixture
product_sha="$(git -C "$product" rev-parse HEAD)"
git init --quiet --bare "$product_remote"
git -C "$product" push --quiet "$product_remote" \
  "$product_sha:refs/heads/test-head"
sudo -n chown -R root:root "$product_forge_root"

# Bootstrap mode is explicit and accepts only the current named branch after
# that exact branch has been published into its origin-tracking ref.
git -C "$control" switch --quiet -c codex/host-ci-unpublished-test
git -C "$control" commit --quiet --allow-empty -m 'unpublished bootstrap probe'
if JAIN_HOST_CI_BOOTSTRAP_REF=refs/heads/codex/host-ci-unpublished-test \
  JAIN_HOST_CI_SANDBOX="$sandbox" JAIN_SPLIT_ROOT="$sandbox_family_root" \
    "$control/ops/ci/split-host-ci.sh" \
      veox jain-report "$product_sha" "$product" jain-report/required \
      >"$tmp/unpublished-bootstrap.log" 2>&1; then
  printf 'parent accepted an unpublished bootstrap control branch\n' >&2
  exit 1
fi
grep -Eq 'cannot resolve the published control-plane authority ref|exact control-plane integrity check failed' \
  "$tmp/unpublished-bootstrap.log"
git -C "$control" switch --quiet codex/host-ci-bootstrap-test

# The root-only bootstrap authority is closed and short-lived. Missing,
# mismatched, expired, or overlong fields fail before control materialization,
# worker startup, or publication.
valid_sandbox_config="$tmp/valid-sandbox-config.json"
sudo -n cat "$sandbox_config" >"$valid_sandbox_config"
for bootstrap_case in missing-commit wrong-commit expired overlong; do
  case "$bootstrap_case" in
    missing-commit)
      sudo -n jq 'del(.bootstrap_commit)' "$sandbox_config" \
        >"$tmp/bootstrap-reject.json"
      expected_bootstrap_failure='invalid bootstrap control authority'
      ;;
    wrong-commit)
      sudo -n jq '.bootstrap_commit = "0000000000000000000000000000000000000000"' \
        "$sandbox_config" >"$tmp/bootstrap-reject.json"
      expected_bootstrap_failure='bootstrap control commit differs from the exact request'
      ;;
    expired)
      sudo -n jq '.bootstrap_expires_at = "1"' "$sandbox_config" \
        >"$tmp/bootstrap-reject.json"
      expected_bootstrap_failure='bootstrap control authority is expired or exceeds two hours'
      ;;
    overlong)
      jq --arg expires "$(( $(date +%s) + 10800 ))" \
        '.bootstrap_expires_at = $expires' "$valid_sandbox_config" \
        >"$tmp/bootstrap-reject.json"
      expected_bootstrap_failure='bootstrap control authority is expired or exceeds two hours'
      ;;
  esac
  sudo -n install -o root -g root -m 0600 \
    "$tmp/bootstrap-reject.json" "$sandbox_config"
  bootstrap_reject_log="$tmp/bootstrap-$bootstrap_case.log"
  if JAIN_HOST_CI_SANDBOX="$sandbox" \
    JAIN_SPLIT_ROOT="$sandbox_family_root" \
      "$control/ops/ci/split-host-ci.sh" \
        veox jain-report "$product_sha" "$product" \
        jain-report/required >"$bootstrap_reject_log" 2>&1; then
    printf 'sandbox accepted %s bootstrap authority\n' "$bootstrap_case" >&2
    exit 1
  fi
  grep -Fq "$expected_bootstrap_failure" "$bootstrap_reject_log" || {
    cat "$bootstrap_reject_log" >&2
    printf 'sandbox did not reject %s bootstrap authority at its root gate\n' \
      "$bootstrap_case" >&2
    exit 1
  }
  sudo -n install -o root -g root -m 0600 \
    "$valid_sandbox_config" "$sandbox_config"
done

# A caller-local commit is not product authority. Keeping it as the caller's
# HEAD also proves the successful run below is staged from the forge ref.
printf 'caller-only\n' >"$product/caller-only.txt"
git -C "$product" add caller-only.txt
git -C "$product" commit --quiet -m 'caller-only unadvertised commit'
unadvertised_sha="$(git -C "$product" rev-parse HEAD)"

# Caller-provided mode markers and caller-provided credentials are both
# rejected before a candidate starts. The cleanup victim must survive.
victim="$tmp/codex-preseed-victim"
mkdir -p "$victim"
printf 'preserve\n' >"$victim/sentinel"
credential_reject_log="$tmp/credential-reject.log"
if JAIN_HOST_CI_EXACT_ROOT="$control" \
  JAIN_HOST_CI_SOURCE_ROOT="$control" \
  JAIN_HOST_CI_CONTROL_COMMIT="$control_commit" \
  JAIN_HOST_CI_BOOTSTRAP_ROOT="$victim" \
  JERYU_MERGE_TOKEN=caller-must-not-retain-this-token \
  JAIN_HOST_CI_PUBLISHER="$publisher" \
  JAIN_SPLIT_ROOT="$split_root" \
    "$control/ops/ci/split-host-ci.sh" \
      veox jain-report "$product_sha" "$tmp/not-a-repository" \
      jain-report/required >"$credential_reject_log" 2>&1; then
  printf 'host CI accepted caller credentials or preseeded mode markers\n' >&2
  exit 1
fi
grep -Fq 'caller-provided forge credentials are forbidden' "$credential_reject_log" || {
  cat "$credential_reject_log" >&2
  printf 'host CI did not fail at the credential boundary\n' >&2
  exit 1
}
[[ "$(<"$victim/sentinel")" == preserve ]] || {
  printf 'credential rejection started work or deleted a caller path\n' >&2
  exit 1
}

# A structurally valid self-pointed reviewed child can return only a local
# result. It has no publisher code path and no credential.
mkdir -p "$split_root/target/host-ci-sandboxes"
forged_root="$(mktemp -d "$split_root/target/host-ci-sandboxes/split-host-ci-bootstrap.XXXXXX")"
chmod 0700 "$forged_root"
git clone --quiet --no-local --no-checkout "$control" "$forged_root/control-plane"
git -C "$forged_root/control-plane" checkout --quiet --detach "$control_commit"
git -C "$forged_root/control-plane" remote remove origin
git -C "$control" show "$control_commit:ops/ci/split-host-ci.sh" \
  >"$forged_root/.split-host-ci-reviewed"
chmod 0500 "$forged_root/.split-host-ci-reviewed"
forged_seal="$(printf 'ab%.0s' {1..32})"
forged_result="$forged_root/child-result.json"
jq -n --arg seal "$forged_seal" --arg source_root "$control" \
  --arg exact_root "$forged_root/control-plane" \
  --arg commit "$control_commit" --arg result_path "$forged_result" \
  '{schema_version:"jain.host-ci-reexec/v3",seal:$seal,
    source_root:$source_root,exact_root:$exact_root,commit:$commit,
    result_path:$result_path,
    splitctl_path:"/opt/jain-ci/authority/splitctl"}' \
  >"$forged_root/reexec-state.json"
chmod 0600 "$forged_root/reexec-state.json"
forged_started="$tmp/forged-started"
forged_continue="$tmp/forged-continue"
touch "$forged_continue"
forge_lines_before="$(wc -l <"$forge_log" 2>/dev/null || echo 0)"
if JAIN_HOST_CI_REEXEC_STATE="$forged_root/reexec-state.json" \
  JAIN_HOST_CI_REEXEC_SEAL="$forged_seal" \
  JAIN_SPLIT_ROOT="$split_root" \
  CARGO_TARGET_DIR="$repo_root/target" \
    bash "$forged_root/.split-host-ci-reviewed" \
      veox jain-report "$product_sha" "$product" jain-report/required \
      >/dev/null 2>&1; then
  printf 'reviewed child accepted a caller-owned legacy state/receipt\n' >&2
  exit 1
fi
[[ ! -e "$forged_result" ]] || {
  printf 'rejected reviewed child still produced caller-owned evidence\n' >&2
  exit 1
}
forge_lines_after="$(wc -l <"$forge_log" 2>/dev/null || echo 0)"
[[ "$forge_lines_after" == "$forge_lines_before" ]] || {
  printf 'direct reviewed child reached the forge\n' >&2
  exit 1
}
[[ "$(<"$victim/sentinel")" == preserve && -d "$forged_root" ]] || {
  printf 'direct reviewed child deleted caller-controlled paths\n' >&2
  exit 1
}
rm -rf -- "$forged_root"
forged_root=""

# A commit that exists only in the caller checkout is rejected before any
# candidate starts or any forge publication occurs.
unadvertised_offset="$(stat -c '%s' "$forge_log")"
if JAIN_HOST_CI_SANDBOX="$sandbox" \
  JAIN_SPLIT_ROOT="$sandbox_family_root" \
    "$control/ops/ci/split-host-ci.sh" \
      veox jain-report "$unadvertised_sha" "$product" \
      jain-report/required >"$tmp/unadvertised.log" 2>&1; then
  printf 'sandbox accepted a caller-only product commit\n' >&2
  exit 1
fi
grep -Fq 'requested head is not an advertised product ref' \
  "$tmp/unadvertised.log" || {
  cat "$tmp/unadvertised.log" >&2
  printf 'sandbox did not reject caller-only product authority\n' >&2
  exit 1
}
[[ "$(stat -c '%s' "$forge_log")" == "$unadvertised_offset" ]] || {
  printf 'caller-only product rejection reached the forge\n' >&2
  exit 1
}

# Full parent -> reviewed runner -> candidate path. The candidate scans every
# visible proc entry and the root config path, then makes a real forged-status
# request. It must recover nothing and the request must not reach the server.
success_log="$tmp/success-runner.log"
success_offset="$(stat -c '%s' "$forge_log")"
host_pid_namespace="$(readlink /proc/self/ns/pid)"
host_user_namespace="$(readlink /proc/self/ns/user)"
JAIN_HOST_CI_PUBLISHER="$publisher" \
JAIN_HOST_CI_SANDBOX="$sandbox" \
JAIN_SPLIT_ROOT="$sandbox_family_root" \
JAIN_RELEASE_CI=0 \
JAIN_RUSTSEC_ADVISORY_SOURCE=/caller/forbidden-advisory-source \
JAIN_TEST_ATTACK_URL="$forge_base" \
JAIN_TEST_REQUIRE_ISOLATION=1 \
JAIN_TEST_HOST_PID_NAMESPACE="$host_pid_namespace" \
JAIN_TEST_HOST_USER_NAMESPACE="$host_user_namespace" \
JAIN_TEST_ROOT_CONFIG_PATH="$publisher_config" \
JAIN_TEST_ROOT_REQUEST_PATH="$request_root" \
JAIN_TEST_FS_MONITOR_PATH=/opt/jain-ci/cargo-home/fsmonitor-attack.sh \
  "$control/ops/ci/split-host-ci.sh" \
    veox jain-report "$product_sha" "$product" jain-report/required \
    >"$success_log" 2>&1 || {
  cat "$success_log" >&2
  printf 'validated parent could not publish success\n' >&2
  exit 1
}
success_tail="$(tail -c "+$((success_offset + 1))" "$forge_log")"
grep -Fq '"name":"jain-report/required"' <<<"$success_tail" \
  && grep -Fq '"conclusion":"success"' <<<"$success_tail" || {
  cat "$forge_log" >&2
  printf 'publisher did not publish a success check\n' >&2
  exit 1
}
grep -Fq '"name":"jankurai/proof"' <<<"$success_tail" \
  && grep -Fq 'receipt_sha256=' <<<"$success_tail" \
  && grep -Fq 'attempt_id=' <<<"$success_tail" || {
  printf 'publisher did not publish SHA-bound proof evidence\n' >&2
  exit 1
}
proof_post_line="$(grep -nF '"name":"jankurai/proof"' \
  <<<"$success_tail" | head -1 | cut -d: -f1)"
proof_get_line="$(grep -nF \
  "GET /repos/veox/jain-report/commits/$product_sha/check-runs HTTP/1.1" \
  <<<"$success_tail" | head -1 | cut -d: -f1)"
required_post_line="$(grep -nF '"name":"jain-report/required"' \
  <<<"$success_tail" | head -1 | cut -d: -f1)"
required_get_line="$(grep -nF \
  "GET /repos/veox/jain-report/commits/$product_sha/check-runs HTTP/1.1" \
  <<<"$success_tail" | sed -n '2p' | cut -d: -f1)"
status_post_line="$(grep -nF \
  "POST /repos/veox/jain-report/statuses/$product_sha HTTP/1.1" \
  <<<"$success_tail" | head -1 | cut -d: -f1)"
status_get_line="$(grep -nF \
  "GET /repos/veox/jain-report/commits/$product_sha/status HTTP/1.1" \
  <<<"$success_tail" | head -1 | cut -d: -f1)"
[[ "$proof_post_line" =~ ^[0-9]+$ && "$proof_get_line" =~ ^[0-9]+$ \
  && "$required_post_line" =~ ^[0-9]+$ && "$required_get_line" =~ ^[0-9]+$ \
  && "$status_post_line" =~ ^[0-9]+$ && "$status_get_line" =~ ^[0-9]+$ ]] \
  && (( proof_post_line < proof_get_line \
    && proof_get_line < required_post_line \
    && required_post_line < required_get_line \
    && required_get_line < status_post_line \
    && status_post_line < status_get_line )) || {
  printf 'publisher did not enforce exact POST/readback publication order\n' >&2
  exit 1
}
grep -Fq '"state":"success"' "$forge_log" || {
  printf 'publisher did not publish a success status\n' >&2
  exit 1
}
grep -Fq 'root-seal=' "$forge_log" || {
  printf 'publisher success did not bind the root one-shot seal\n' >&2
  exit 1
}
sudo -n jq -e 'has("token") | not' "$publisher_config" >/dev/null || {
  printf 'publisher configuration retained an embedded credential\n' >&2
  exit 1
}
sudo -n jq -e 'has("token") | not' "$sandbox_config" >/dev/null || {
  printf 'sandbox configuration retained an embedded credential\n' >&2
  exit 1
}
if printf '%s\n' "$publisher_token" \
  | sudo -n grep -R -F -l -f - -- "$publisher_config" "$request_root" \
    "$native_evidence_root" "$proof_evidence_root" "$success_log" \
    "$runner_log" "$forge_log" "$tmp/forge.stderr" >/dev/null 2>&1; then
  printf 'publisher credential escaped into config, receipts, evidence, or logs\n' >&2
  exit 1
fi
unset publisher_token
grep -Fq 'boundary_no_new_privs=1' "$success_log"
grep -Fq 'boundary_cap_inheritable=0000000000000000' "$success_log"
grep -Fq 'boundary_cap_permitted=0000000000000000' "$success_log"
grep -Fq 'boundary_cap_effective=0000000000000000' "$success_log"
grep -Fq 'boundary_cap_bounding=0000000000000000' "$success_log"
grep -Fq 'boundary_cap_ambient=0000000000000000' "$success_log"
grep -Fq 'boundary_immutable_control_write=0' "$success_log"
grep -Fq 'boundary_pid_namespace_separate=1' "$success_log"
grep -Fq 'boundary_user_namespace_separate=1' "$success_log"
grep -Eq 'boundary_visible_pids=[1-9][0-9]?$' "$success_log"
grep -Fq 'nested PID/user namespaces established' "$success_log"
grep -Fq 'boundary_credential_recovered=0' "$success_log"
grep -Fq 'boundary_root_config_readable=0' "$success_log"
grep -Fq 'boundary_root_request_readable=0' "$success_log"
grep -Fq 'boundary_forge_attempt=blocked' "$success_log"
grep -Fq 'boundary_sudo_attempt=blocked' "$success_log"
grep -Fq 'boundary_release_ci=1' "$success_log"
grep -Fq 'boundary_rustsec_standalone=1' "$success_log"
grep -Fq 'boundary_deny_db_physical=1' "$success_log"
grep -Fq 'boundary_advisory_swap_blocked=1' "$success_log"
grep -Fq 'boundary_bare_mirror_safe=1' "$success_log"
grep -Fq 'boundary_evidence_staging_bounded=1' "$success_log"
grep -Fq 'boundary_survivor_started=1' "$success_log"
grep -Fq 'worker cgroup stopped before sealing' "$success_log"
if grep -Fq '/candidate-forge' "$forge_log"; then
  printf 'candidate forged a request through the isolated boundary\n' >&2
  exit 1
fi
if sudo -n grep -Fxq 0 "$worker_cache/fsmonitor-attack.sh.log" 2>/dev/null; then
  printf 'root publisher executed worker-controlled Git fsmonitor config\n' >&2
  exit 1
fi
[[ ! -e "$worker_cache/fsmonitor-attack.sh.log" ]] || {
  printf 'worker-controlled fsmonitor executed despite immutable config overrides\n' >&2
  exit 1
}
# The successful request is root-owned and consumed. The parent cannot execute
# the publisher, and even this test host's broad sudo cannot replay it.
mapfile -t retained_states < <(
  sudo -n find "$request_root" -mindepth 2 -maxdepth 2 \
    -type f -name root-state.json -print
)
[[ "${#retained_states[@]}" == 1 ]] || {
  printf 'sandbox did not retain exactly one sealed root request fixture\n' >&2
  exit 1
}
success_request="$(dirname "${retained_states[0]}")"
sudo -n jq -e 'select(.status == "consumed")' \
  "$success_request/root-state.json" >/dev/null
sudo -n jq -e --arg ref "$bootstrap_control_ref" \
  --arg expires "$bootstrap_expires_at" '
    select(.control_ref == $ref and .bootstrap_expires_at == $expires)' \
  "$success_request/root-state.json" >/dev/null
success_proof_dir="$(sudo -n jq -er '.proof_evidence_dir' \
  "$success_request/root-result.json")"
case "$success_proof_dir" in
  "$proof_evidence_root"/*) ;;
  *) printf 'proof evidence escaped its durable root\n' >&2; exit 1 ;;
esac
[[ "$(sudo -n stat -c '%u:%g:%a' "$success_proof_dir")" == '0:0:500' \
  && "$(sudo -n stat -c '%u:%g:%a:%h' "$success_proof_dir/report.json")" \
    == '0:0:400:1' \
  && "$(sudo -n stat -c '%u:%g:%a:%h' "$success_proof_dir/receipt.json")" \
    == '0:0:400:1' ]] || {
  printf 'durable proof evidence lacks immutable root ownership\n' >&2
  exit 1
}
sudo -n jq -e --arg repo jain-report --arg head "$product_sha" '
  select(.schema_version == "jain.jankurai-exact-sha-evidence/v1")
  | select(.status == "pass" and .repository == $repo and .commit == $head)
  | select(.hard_findings == 0 and .caps_applied == 0)
  | select(.ratchet_passed == true)
  | select(.clean_tracked_tree_at_start == true
      and .clean_tracked_tree_at_finish == true)' \
  "$success_proof_dir/receipt.json" >/dev/null
sudo -n jq -e '
  select(.fixture.source_read_only == true
    and .fixture.network_isolated == true)' \
  "$success_proof_dir/report.json" >/dev/null
# Installed protocol versions are mandatory trust inputs, not advisory parser
# hints. A v4 publisher config is rejected even for an otherwise sealed v4
# request, and restoring the exact v5 bytes does not make that request replayable.
sudo -n cp -- "$publisher_config" "$tmp/publisher-config.v5"
sudo -n jq '.schema_version="jain.host-ci-publisher-config/v4"' \
  "$publisher_config" >"$tmp/publisher-config.v4"
sudo -n install -o root -g root -m 0600 \
  "$tmp/publisher-config.v4" "$publisher_config"
if sudo -n "$publisher" "$success_request" \
  >"$tmp/old-publisher-config.log" 2>&1; then
  printf 'publisher accepted a v4 config protocol\n' >&2
  exit 1
fi
grep -Fq 'invalid publisher config schema' "$tmp/old-publisher-config.log"
sudo -n install -o root -g root -m 0600 \
  "$tmp/publisher-config.v5" "$publisher_config"
if "$publisher" "$success_request" >/dev/null 2>&1; then
  printf 'unprivileged parent directly executed the root-only publisher\n' >&2
  exit 1
fi
forge_offset="$(stat -c '%s' "$forge_log")"
if sudo -n "$publisher" "$success_request" >"$tmp/replay.log" 2>&1; then
  printf 'publisher replayed a consumed root request\n' >&2
  exit 1
fi
grep -Fq 'already used or is being published' "$tmp/replay.log"
[[ "$(stat -c '%s' "$forge_log")" == "$forge_offset" ]] || {
  printf 'replayed publisher request reached the forge\n' >&2
  exit 1
}

# A caller-owned success/receipt directory is not root publication authority.
forged_publish="$tmp/forged-publish"
mkdir -m 0700 "$forged_publish"
printf '{"conclusion":"success"}\n' >"$forged_publish/root-result.json"
if sudo -n "$publisher" "$forged_publish" >"$tmp/forged-publish.log" 2>&1; then
  printf 'publisher accepted a caller-owned forged receipt\n' >&2
  exit 1
fi
grep -Fq 'outside root authority' "$tmp/forged-publish.log"

make_sealed_variant() {
  local request_id="$1" result_filter="$2" sealed_at="$3"
  local destination="$request_root/$request_id"
  local local_result="$tmp/$request_id.result.json"
  local local_state="$tmp/$request_id.state.json"
  local nonce proof_receipt_sha result_sha root_seal
  sudo -n cp -a -- "$success_request" "$destination"
  sudo -n rm -rf -- "$destination/publish.lock"
  sudo -n jq --arg request_id "$request_id" "$result_filter" \
    "$success_request/root-result.json" >"$local_result"
  result_sha="$(sha256sum -- "$local_result" | cut -d' ' -f1)"
  proof_receipt_sha="$(jq -er '.proof_receipt_sha256' "$local_result")"
  nonce="$(sudo -n jq -er '.nonce' "$success_request/root-state.json")"
  root_seal="$({
    printf '%s\n%s\n%s\n%s\n%s\n' \
      "$nonce" "$result_sha" "$proof_receipt_sha" "$sealed_at" "$request_id"
  } | sha256sum | cut -d' ' -f1)"
  sudo -n jq --arg request_id "$request_id" --arg status sealed \
    --arg result_sha "$result_sha" --arg root_seal "$root_seal" \
    --argjson sealed_at "$sealed_at" \
    '.request_id=$request_id | .status=$status
     | .result_sha256=$result_sha | .root_seal=$root_seal
     | .sealed_at=$sealed_at' "$success_request/root-state.json" >"$local_state"
  sudo -n install -o root -g root -m 0600 "$local_result" \
    "$destination/root-result.json"
  sudo -n install -o root -g root -m 0600 "$local_state" \
    "$destination/root-state.json"
}

# A correctly recomputed but old root seal is stale and becomes one-shot even
# on rejection.
stale_id="$(printf 'a%.0s' {1..64})"
make_sealed_variant "$stale_id" '.request_id=$request_id' \
  "$(( $(date +%s) - 1000 ))"
if sudo -n "$publisher" "$request_root/$stale_id" \
  >"$tmp/stale.log" 2>&1; then
  printf 'publisher accepted a stale root nonce/seal\n' >&2
  exit 1
fi
grep -Fq 'root request seal is stale' "$tmp/stale.log"
if sudo -n "$publisher" "$request_root/$stale_id" >/dev/null 2>&1; then
  printf 'publisher replayed a stale rejected nonce\n' >&2
  exit 1
fi

# Publisher authority must equal the expiry that the sandbox normalized and
# sealed into root state; changing only that state field fails before a POST.
expiry_mismatch_id="$(printf '8%.0s' {1..64})"
make_sealed_variant "$expiry_mismatch_id" '.request_id=$request_id' \
  "$(date +%s)"
sudo -n jq '.bootstrap_expires_at = "1"' \
  "$request_root/$expiry_mismatch_id/root-state.json" \
  >"$tmp/expiry-mismatch.state.json"
sudo -n install -o root -g root -m 0600 \
  "$tmp/expiry-mismatch.state.json" \
  "$request_root/$expiry_mismatch_id/root-state.json"
expiry_mismatch_offset="$(stat -c '%s' "$forge_log")"
if sudo -n "$publisher" "$request_root/$expiry_mismatch_id" \
  >"$tmp/expiry-mismatch.log" 2>&1; then
  printf 'publisher accepted a mismatched bootstrap expiry\n' >&2
  exit 1
fi
grep -Fq 'bootstrap expiry differs across root artifacts' \
  "$tmp/expiry-mismatch.log"
[[ "$(stat -c '%s' "$forge_log")" == "$expiry_mismatch_offset" ]] || {
  printf 'bootstrap expiry mismatch reached the forge\n' >&2
  exit 1
}

old_result_id="$(printf 'c%.0s' {1..64})"
make_sealed_variant "$old_result_id" \
  '.request_id=$request_id
   | .schema_version="jain.host-ci-root-result/v3"' "$(date +%s)"
if sudo -n "$publisher" "$request_root/$old_result_id" \
  >"$tmp/old-root-result.log" 2>&1; then
  printf 'v4 publisher accepted a v3 root result protocol\n' >&2
  exit 1
fi
grep -Fq 'invalid root result schema' "$tmp/old-root-result.log"

# Even a valid fresh root seal cannot mark native evidence optional for a repo
# whose reviewed native policy requires it.
downgrade_id="$(printf 'b%.0s' {1..64})"
make_sealed_variant "$downgrade_id" \
  '.request_id=$request_id | .repository="jain-core"
   | .required_check="jain-core/required"
   | .native_evidence_required=false' "$(date +%s)"
if sudo -n "$publisher" "$request_root/$downgrade_id" \
  >"$tmp/downgrade.log" 2>&1; then
  printf 'publisher accepted a child native-policy downgrade\n' >&2
  exit 1
fi
grep -Fq 'native evidence policy downgrade' "$tmp/downgrade.log"
[[ "$(stat -c '%s' "$forge_log")" == "$forge_offset" ]] || {
  printf 'replay/stale/downgrade rejection reached the forge\n' >&2
  exit 1
}

run_fixture_lane() {
  local log="${1:?log required}"
  shift
  env \
    JAIN_HOST_CI_SANDBOX="$sandbox" \
    JAIN_SPLIT_ROOT="$sandbox_family_root" \
    JAIN_TEST_ATTACK_URL="$forge_base" \
    JAIN_TEST_REQUIRE_ISOLATION=1 \
    JAIN_TEST_HOST_PID_NAMESPACE="$host_pid_namespace" \
    JAIN_TEST_HOST_USER_NAMESPACE="$host_user_namespace" \
    JAIN_TEST_ROOT_CONFIG_PATH="$publisher_config" \
    JAIN_TEST_ROOT_REQUEST_PATH="$request_root" \
    JAIN_TEST_FS_MONITOR_PATH=/opt/jain-ci/cargo-home/fsmonitor-attack.sh \
    JAIN_RUSTSEC_ADVISORY_SOURCE=/caller/forbidden-advisory-source \
    "$@" \
    "$control/ops/ci/split-host-ci.sh" \
      veox jain-report "$product_sha" "$product" jain-report/required \
      >"$log" 2>&1
}

latest_root_request() {
  sudo -n find "$request_root" -mindepth 2 -maxdepth 2 \
    -type f -name root-state.json -printf '%T@ %h\n' \
    | LC_ALL=C sort -nr | head -1 | cut -d' ' -f2-
}

exercise_partial_publication() {
  local behavior="${1:?behavior required}"
  local expected_state="${2:?state required}"
  local stage="${3:?stage required}"
  local offset tail request replay_offset
  # Every injected transition starts from a clean fake-forge state. Otherwise
  # a prior successful run can make a later proof readback appear to contain a
  # required check and collapse the intended state-machine boundary.
  printf '' >"$forge_state"
  printf '%s\n' "$behavior" >"$forge_behavior"
  offset="$(stat -c '%s' "$forge_log")"
  if run_fixture_lane "$tmp/$behavior.log"; then
    printf 'partial publication fixture unexpectedly succeeded: %s\n' \
      "$behavior" >&2
    exit 1
  fi
  tail="$(tail -c "+$((offset + 1))" "$forge_log")"
  grep -Fq '"name":"jankurai/proof"' <<<"$tail" || {
    printf '%s did not attempt proof publication\n' "$behavior" >&2
    exit 1
  }
  if (( stage >= 2 )); then
    grep -Fq \
      "GET /repos/veox/jain-report/commits/$product_sha/check-runs HTTP/1.1" \
      <<<"$tail"
  elif grep -Fq \
      "GET /repos/veox/jain-report/commits/$product_sha/check-runs HTTP/1.1" \
      <<<"$tail"; then
    printf '%s read back a failed proof POST\n' "$behavior" >&2
    exit 1
  fi
  if (( stage >= 3 )); then
    grep -Fq '"name":"jain-report/required"' <<<"$tail"
  elif grep -Fq '"name":"jain-report/required"' <<<"$tail"; then
    printf '%s reached required-check publication too early\n' "$behavior" >&2
    exit 1
  fi
  if (( stage >= 4 )); then
    [[ "$(grep -Fc \
      "GET /repos/veox/jain-report/commits/$product_sha/check-runs HTTP/1.1" \
      <<<"$tail")" -ge 2 ]]
  elif [[ "$(grep -Fc \
    "GET /repos/veox/jain-report/commits/$product_sha/check-runs HTTP/1.1" \
    <<<"$tail")" -ge 2 ]]; then
    printf '%s reached required-check readback too early\n' "$behavior" >&2
    exit 1
  fi
  if (( stage >= 5 )); then
    grep -Fq \
      "POST /repos/veox/jain-report/statuses/$product_sha HTTP/1.1" \
      <<<"$tail"
  elif grep -Fq \
      "POST /repos/veox/jain-report/statuses/$product_sha HTTP/1.1" \
      <<<"$tail"; then
    printf '%s reached commit-status publication too early\n' "$behavior" >&2
    exit 1
  fi
  if (( stage >= 6 )); then
    grep -Fq \
      "GET /repos/veox/jain-report/commits/$product_sha/status HTTP/1.1" \
      <<<"$tail"
  elif grep -Fq \
    "GET /repos/veox/jain-report/commits/$product_sha/status HTTP/1.1" \
    <<<"$tail"; then
    printf '%s reached commit-status readback too early\n' "$behavior" >&2
    exit 1
  fi
  request="$(latest_root_request)"
  sudo -n jq -e --arg state "$expected_state" \
    'select(.status == $state)' "$request/root-state.json" >/dev/null
  replay_offset="$(stat -c '%s' "$forge_log")"
  if sudo -n "$publisher" "$request" >/dev/null 2>&1; then
    printf 'partial publication was replayable: %s\n' "$behavior" >&2
    exit 1
  fi
  [[ "$(stat -c '%s' "$forge_log")" == "$replay_offset" ]] || {
    printf 'partial publication replay reached forge: %s\n' "$behavior" >&2
    exit 1
  }
  printf 'ok\n' >"$forge_behavior"
}

# Proof POST is the publication gate. Once its request is attempted, that and
# every later readback/required/status failure consumes the request permanently.
exercise_partial_publication proof-post-fail consumed 1
exercise_partial_publication readback-missing consumed 2
exercise_partial_publication readback-mismatch consumed 2
exercise_partial_publication required-post-fail consumed 3
exercise_partial_publication required-readback-missing consumed 4
exercise_partial_publication status-post-fail consumed 5
exercise_partial_publication status-readback-missing consumed 6
printf '' >"$forge_state"

# A nonzero reviewed worker exit is independently sealed and can publish only
# failure; it cannot reuse the prior success result.
failure_offset="$(stat -c '%s' "$forge_log")"
if JAIN_HOST_CI_SANDBOX="$sandbox" \
  JAIN_SPLIT_ROOT="$sandbox_family_root" \
  JAIN_TEST_ATTACK_URL="$forge_base" \
  JAIN_TEST_REQUIRE_ISOLATION=1 \
  JAIN_TEST_HOST_PID_NAMESPACE="$host_pid_namespace" \
  JAIN_TEST_HOST_USER_NAMESPACE="$host_user_namespace" \
  JAIN_TEST_ROOT_CONFIG_PATH="$publisher_config" \
  JAIN_TEST_ROOT_REQUEST_PATH="$request_root" \
  JAIN_TEST_FS_MONITOR_PATH=/opt/jain-ci/cargo-home/fsmonitor-attack.sh \
  JAIN_RUSTSEC_ADVISORY_SOURCE=/caller/forbidden-advisory-source \
  JAIN_TEST_FORCE_FAILURE=1 \
    "$control/ops/ci/split-host-ci.sh" \
      veox jain-report "$product_sha" "$product" jain-report/required \
      >"$tmp/failure-run.log" 2>&1; then
  printf 'nonzero worker run returned publication success\n' >&2
  exit 1
fi
failure_tail="$(tail -c "+$((failure_offset + 1))" "$forge_log")"
grep -Fq '"conclusion":"failure"' <<<"$failure_tail"
grep -Fq '"state":"failure"' <<<"$failure_tail"
if grep -Fq '"conclusion":"success"' <<<"$failure_tail" \
  || grep -Fq '"state":"success"' <<<"$failure_tail"; then
  printf 'failed worker published success\n' >&2
  exit 1
fi

# A well-formed exact-head report that fails the governed score gate is still
# durable negative evidence. It must publish proof failure before required
# failure, never acquire a success result, and remain one-shot.
printf 'score-failure\n' >"$product/agent/test-auditor-mode"
git -C "$product" add agent/test-auditor-mode
git -C "$product" commit --quiet -m 'fixture governed score failure'
score_failure_sha="$(git -C "$product" rev-parse HEAD)"
sudo -n /usr/bin/git -C "$product" push --quiet "$product_remote" \
  "$score_failure_sha:refs/heads/score-failure"
score_failure_offset="$(stat -c '%s' "$forge_log")"
if env \
  JAIN_HOST_CI_SANDBOX="$sandbox" \
  JAIN_SPLIT_ROOT="$sandbox_family_root" \
  JAIN_TEST_ATTACK_URL="$forge_base" \
  JAIN_TEST_REQUIRE_ISOLATION=1 \
  JAIN_TEST_HOST_PID_NAMESPACE="$host_pid_namespace" \
  JAIN_TEST_HOST_USER_NAMESPACE="$host_user_namespace" \
  JAIN_TEST_ROOT_CONFIG_PATH="$publisher_config" \
  JAIN_TEST_ROOT_REQUEST_PATH="$request_root" \
  JAIN_TEST_FS_MONITOR_PATH=/opt/jain-ci/cargo-home/fsmonitor-attack.sh \
  JAIN_RUSTSEC_ADVISORY_SOURCE=/caller/forbidden-advisory-source \
  "$control/ops/ci/split-host-ci.sh" \
    veox jain-report "$score_failure_sha" "$product" \
    jain-report/required >"$tmp/score-failure.log" 2>&1; then
  printf 'governed score failure returned publication success\n' >&2
  exit 1
fi
score_failure_tail="$(tail -c "+$((score_failure_offset + 1))" "$forge_log")"
grep -Fq '"name":"jankurai/proof"' <<<"$score_failure_tail"
grep -Fq 'proof_status=fail' <<<"$score_failure_tail"
grep -Fq '"conclusion":"failure"' <<<"$score_failure_tail"
grep -Fq '"name":"jain-report/required"' <<<"$score_failure_tail"
score_proof_line="$(grep -nF '"name":"jankurai/proof"' \
  <<<"$score_failure_tail" | head -n 1 | cut -d: -f1)"
score_required_line="$(grep -nF '"name":"jain-report/required"' \
  <<<"$score_failure_tail" | head -n 1 | cut -d: -f1)"
[[ "$score_proof_line" =~ ^[0-9]+$ && "$score_required_line" =~ ^[0-9]+$ \
  && (( score_proof_line < score_required_line )) ]] || {
  printf 'governed score failure did not publish proof before required\n' >&2
  exit 1
}
if grep -Fq '"conclusion":"success"' <<<"$score_failure_tail" \
  || grep -Fq '"state":"success"' <<<"$score_failure_tail"; then
  printf 'governed score failure published success\n' >&2
  exit 1
fi

# Repositories without a governed baseline use their policy floor directly.
# The isolated auditor must not synthesize a baseline or reject that shape.
git -C "$product" rm --quiet agent/test-auditor-mode \
  agent/jankurai-baseline.json
git -C "$product" commit --quiet -m 'fixture governed floor only'
floor_only_sha="$(git -C "$product" rev-parse HEAD)"
sudo -n /usr/bin/git -C "$product" push --quiet "$product_remote" \
  "$floor_only_sha:refs/heads/floor-only"
floor_only_offset="$(stat -c '%s' "$forge_log")"
env \
  JAIN_HOST_CI_SANDBOX="$sandbox" \
  JAIN_SPLIT_ROOT="$sandbox_family_root" \
  JAIN_TEST_ATTACK_URL="$forge_base" \
  JAIN_TEST_REQUIRE_ISOLATION=1 \
  JAIN_TEST_HOST_PID_NAMESPACE="$host_pid_namespace" \
  JAIN_TEST_HOST_USER_NAMESPACE="$host_user_namespace" \
  JAIN_TEST_ROOT_CONFIG_PATH="$publisher_config" \
  JAIN_TEST_ROOT_REQUEST_PATH="$request_root" \
  JAIN_TEST_FS_MONITOR_PATH=/opt/jain-ci/cargo-home/fsmonitor-attack.sh \
  JAIN_RUSTSEC_ADVISORY_SOURCE=/caller/forbidden-advisory-source \
  "$control/ops/ci/split-host-ci.sh" \
    veox jain-report "$floor_only_sha" "$product" \
    jain-report/required >"$tmp/floor-only.log" 2>&1
floor_only_tail="$(tail -c "+$((floor_only_offset + 1))" "$forge_log")"
grep -Fq 'proof_status=pass' <<<"$floor_only_tail"
grep -Fq '"conclusion":"success"' <<<"$floor_only_tail"

# An advertised product head still has no authority to forge the report's
# audited SHA. The validator rejects it before a root result or forge POST.
printf 'wrong-head\n' >"$product/agent/test-auditor-mode"
git -C "$product" add agent/test-auditor-mode
git -C "$product" commit --quiet -m 'fixture forged auditor report'
forged_report_sha="$(git -C "$product" rev-parse HEAD)"
sudo -n /usr/bin/git -C "$product" push --quiet "$product_remote" \
  "$forged_report_sha:refs/heads/forged-report"
forged_report_offset="$(stat -c '%s' "$forge_log")"
if env \
  JAIN_HOST_CI_SANDBOX="$sandbox" \
  JAIN_SPLIT_ROOT="$sandbox_family_root" \
  JAIN_TEST_ATTACK_URL="$forge_base" \
  JAIN_TEST_REQUIRE_ISOLATION=1 \
  JAIN_TEST_HOST_PID_NAMESPACE="$host_pid_namespace" \
  JAIN_TEST_HOST_USER_NAMESPACE="$host_user_namespace" \
  JAIN_TEST_ROOT_CONFIG_PATH="$publisher_config" \
  JAIN_TEST_ROOT_REQUEST_PATH="$request_root" \
  JAIN_TEST_FS_MONITOR_PATH=/opt/jain-ci/cargo-home/fsmonitor-attack.sh \
  JAIN_RUSTSEC_ADVISORY_SOURCE=/caller/forbidden-advisory-source \
  "$control/ops/ci/split-host-ci.sh" \
    veox jain-report "$forged_report_sha" "$product" \
    jain-report/required >"$tmp/forged-report.log" 2>&1; then
  printf 'forged Jankurai report acquired publication authority\n' >&2
  exit 1
fi
grep -Eq 'score report git.head|rev-parse|proof evidence promotion failed' \
  "$tmp/forged-report.log"
[[ "$(stat -c '%s' "$forge_log")" == "$forged_report_offset" ]] || {
  printf 'forged Jankurai report reached the forge\n' >&2
  exit 1
}

# A caller can retain a writable descriptor even after the sandbox changes the
# bootstrap tree to the worker identity. Mutating the request through that
# descriptor must not change the already snapshotted authority input. The
# replacement is valid and asks the worker to fail, so a successful run proves
# the sandbox never reread caller-controlled bytes after the ownership change.
mkdir -p "$sandbox_family_root/target/host-ci-sandboxes"
fd_attack_root="$(mktemp -d "$sandbox_family_root/target/host-ci-sandboxes/split-host-ci-bootstrap.XXXXXX")"
chmod 0700 "$fd_attack_root"
mkdir -m 0700 "$fd_attack_root/child-home" \
  "$fd_attack_root/writable" "$fd_attack_root/cargo-target"
fd_attack_request="$fd_attack_root/sandbox-request.json"
jq -cn --arg commit "$control_commit" --arg control_ref "$bootstrap_control_ref" \
  --arg split_root "$sandbox_family_root" \
  --arg owner veox --arg repo jain-report --arg head "$product_sha" \
  --arg product "$fd_attack_root/product-source" \
  --arg check jain-report/required \
  --arg cargo_target "$fd_attack_root/cargo-target" \
  --arg writable "$fd_attack_root/writable" \
  '{schema_version:"jain.host-ci-sandbox-request/v5",
    control_plane_commit:$commit,control_ref:$control_ref,split_root:$split_root,
    arguments:[$owner,$repo,$head,$product,$check],
    environment:{CARGO_TARGET_DIR:$cargo_target,
      JAIN_HOST_CI_WRITABLE_ROOT:$writable,JAIN_SPLIT_ROOT:$split_root,
      JAIN_RELEASE_CI:"1",
      JAIN_TEST_SLEEP_SECONDS:"1"}}' \
  >"$fd_attack_request"
chmod 0600 "$fd_attack_request"
# The other installed broker config is independently strict as well.
sudo -n cp -- "$sandbox_config" "$tmp/sandbox-config.v6"
sudo -n jq '.schema_version="jain.host-ci-sandbox-config/v3"' \
  "$sandbox_config" >"$tmp/sandbox-config.v3"
sudo -n install -o root -g root -m 0600 \
  "$tmp/sandbox-config.v3" "$sandbox_config"
if sudo -n "$sandbox" "$fd_attack_request" \
  >"$tmp/old-sandbox-config.log" 2>&1; then
  printf 'sandbox accepted a v3 config protocol\n' >&2
  exit 1
fi
grep -Fq 'invalid sandbox config schema' "$tmp/old-sandbox-config.log"
sudo -n install -o root -g root -m 0600 \
  "$tmp/sandbox-config.v6" "$sandbox_config"
# The installed v5 request broker rejects a structurally valid request carrying the
# previous protocol before it creates a worker or reaches the forge.
jq '.schema_version="jain.host-ci-sandbox-request/v3"' \
  "$fd_attack_request" >"$tmp/old-sandbox-request.json"
mv "$tmp/old-sandbox-request.json" "$fd_attack_request"
chmod 0600 "$fd_attack_request"
old_request_offset="$(stat -c '%s' "$forge_log")"
if sudo -n "$sandbox" "$fd_attack_request" \
  >"$tmp/old-sandbox-request.log" 2>&1; then
  printf 'v5 sandbox accepted a v3 request protocol\n' >&2
  exit 1
fi
grep -Fq 'invalid sandbox request schema' "$tmp/old-sandbox-request.log"
[[ "$(stat -c '%s' "$forge_log")" == "$old_request_offset" ]] || {
  printf 'old sandbox request protocol reached the forge\n' >&2
  exit 1
}
jq '.schema_version="jain.host-ci-sandbox-request/v5"' \
  "$fd_attack_request" >"$tmp/v5-sandbox-request.json"
mv "$tmp/v5-sandbox-request.json" "$fd_attack_request"
chmod 0600 "$fd_attack_request"

# Root-owned worker bindings cannot be replaced through a crafted parent
# request. Each key is rejected before systemd starts a worker or any request
# reaches the forge.
cp -- "$fd_attack_request" "$tmp/fixed-worker-environment-base.json"
for fixed_key in JAIN_SPLIT_OPS_ROOT JAIN_HOST_CI_REEXEC_STATE \
  JAIN_HOST_CI_NETWORK_ISOLATED JAIN_PINNED_ADVISORY_DB JAIN_ADVISORY_DB \
  JAIN_CARGO_DENY_ADVISORY_DB; do
  fixed_log="$tmp/fixed-worker-environment-$fixed_key.log"
  fixed_forge_offset="$(stat -c '%s' "$forge_log")"
  jq --arg key "$fixed_key" \
    '.environment[$key]="/caller/forbidden-root-authority"' \
    "$tmp/fixed-worker-environment-base.json" >"$fd_attack_request"
  chmod 0600 "$fd_attack_request"
  if sudo -n "$sandbox" "$fd_attack_request" >"$fixed_log" 2>&1; then
    printf 'sandbox accepted caller override of %s\n' "$fixed_key" >&2
    exit 1
  fi
  grep -Fq "forbidden sandbox environment key: $fixed_key" "$fixed_log"
  if grep -Fq 'nested PID/user namespaces established' "$fixed_log"; then
    printf 'sandbox started a worker for forbidden key %s\n' "$fixed_key" >&2
    exit 1
  fi
  [[ "$(stat -c '%s' "$forge_log")" == "$fixed_forge_offset" ]] || {
    printf 'forbidden worker key reached the forge: %s\n' "$fixed_key" >&2
    exit 1
  }
done
cp -- "$tmp/fixed-worker-environment-base.json" "$fd_attack_request"
chmod 0600 "$fd_attack_request"
malicious_request="$(jq -c \
  '.environment.JAIN_TEST_FORCE_FAILURE="1"' "$fd_attack_request")"
attack_complete="$tmp/retained-fd-mutated"
worker_uid="$(id -u xbwork)"
printf '' >"$forge_state"
exec {request_fd}<>"$fd_attack_request"
(
  for _ in $(seq 1 3000); do
    if [[ "$(stat -Lc '%u' "/proc/$BASHPID/fd/$request_fd" 2>/dev/null || true)" \
      == "$worker_uid" ]]; then
      printf '%s\n' "$malicious_request" >&"$request_fd"
      : >"$attack_complete"
      exit 0
    fi
    sleep 0.01
  done
  exit 1
) &
attack_pid=$!
if ! sudo -n "$sandbox" "$fd_attack_request" \
  >"$tmp/retained-fd-run.log" 2>&1; then
  cat "$tmp/retained-fd-run.log" >&2
  printf 'retained descriptor changed snapshotted sandbox authority\n' >&2
  exit 1
fi
if ! wait "$attack_pid"; then
  attack_pid=""
  printf 'retained descriptor attack did not observe worker ownership\n' >&2
  exit 1
fi
attack_pid=""
exec {request_fd}>&-
[[ -f "$attack_complete" ]] \
  && grep -Fq 'JAIN_TEST_FORCE_FAILURE' "$fd_attack_request" || {
  printf 'retained descriptor did not mutate the caller request\n' >&2
  exit 1
}
grep -Fq 'worker cgroup stopped before sealing' "$tmp/retained-fd-run.log"
rm -rf -- "$fd_attack_root"
fd_attack_root=""

make_proof_tamper_variant() {
  local request_id="${1:?request ID required}"
  local mutation="${2:?mutation required}"
  local destination="$request_root/$request_id"
  local proof_parent proof_destination receipt_sha report_sha
  local result_file="$tmp/$request_id.proof-result.json"
  local state_file="$tmp/$request_id.proof-state.json"
  local receipt_file="$tmp/$request_id.proof-receipt.json"
  local nonce result_sha sealed_at root_seal external
  proof_parent="$(dirname "$success_proof_dir")"
  proof_destination="$proof_parent/$request_id"
  sudo -n cp -a -- "$success_request" "$destination"
  sudo -n rm -rf -- "$destination/publish.lock"
  sudo -n install -d -o root -g root -m 0700 "$proof_destination"
  sudo -n install -o root -g root -m 0400 \
    "$success_proof_dir/report.json" "$proof_destination/report.json"
  sudo -n jq --arg report "$proof_destination/report.json" \
    '.report=$report' "$success_proof_dir/receipt.json" >"$receipt_file"
  sudo -n install -o root -g root -m 0400 \
    "$receipt_file" "$proof_destination/receipt.json"
  sudo -n chmod 0500 "$proof_destination"
  receipt_sha="$(sudo -n sha256sum "$proof_destination/receipt.json" \
    | cut -d' ' -f1)"
  report_sha="$(sudo -n sha256sum "$proof_destination/report.json" \
    | cut -d' ' -f1)"
  sudo -n jq --arg request_id "$request_id" \
    --arg proof_dir "$proof_destination" \
    --arg receipt "$proof_destination/receipt.json" \
    --arg receipt_sha "$receipt_sha" \
    --arg report "$proof_destination/report.json" \
    --arg report_sha "$report_sha" '
      .request_id=$request_id
      | .proof_evidence_dir=$proof_dir
      | .proof_receipt_path=$receipt
      | .proof_receipt_sha256=$receipt_sha
      | .proof_report_path=$report
      | .proof_report_sha256=$report_sha' \
    "$success_request/root-result.json" >"$result_file"
  result_sha="$(sha256sum "$result_file" | cut -d' ' -f1)"
  nonce="$(sudo -n jq -er '.nonce' "$success_request/root-state.json")"
  sealed_at="$(date +%s)"
  root_seal="$({
    printf '%s\n%s\n%s\n%s\n%s\n' \
      "$nonce" "$result_sha" "$receipt_sha" "$sealed_at" "$request_id"
  } | sha256sum | cut -d' ' -f1)"
  sudo -n jq --arg request_id "$request_id" --arg status sealed \
    --arg result_sha "$result_sha" --arg root_seal "$root_seal" \
    --argjson sealed_at "$sealed_at" '
      .request_id=$request_id | .status=$status
      | .result_sha256=$result_sha | .root_seal=$root_seal
      | .sealed_at=$sealed_at' "$success_request/root-state.json" >"$state_file"
  sudo -n install -o root -g root -m 0600 \
    "$result_file" "$destination/root-result.json"
  sudo -n install -o root -g root -m 0600 \
    "$state_file" "$destination/root-state.json"
  case "$mutation" in
    missing)
      sudo -n chmod 0700 "$proof_destination"
      sudo -n rm -- "$proof_destination/receipt.json"
      sudo -n chmod 0500 "$proof_destination"
      ;;
    hardlink)
      external="$proof_evidence_root/.hardlink-$request_id"
      sudo -n install -o root -g root -m 0400 \
        "$success_proof_dir/report.json" "$external"
      sudo -n chmod 0700 "$proof_destination"
      sudo -n rm -- "$proof_destination/report.json"
      sudo -n ln -- "$external" "$proof_destination/report.json"
      sudo -n chmod 0500 "$proof_destination"
      ;;
    tamper)
      sudo -n chmod 0700 "$proof_destination"
      sudo -n chmod 0600 "$proof_destination/report.json"
      printf 'tampered\n' | sudo -n tee -a \
        "$proof_destination/report.json" >/dev/null
      sudo -n chmod 0400 "$proof_destination/report.json"
      sudo -n chmod 0500 "$proof_destination"
      ;;
    *) return 1 ;;
  esac
}

# A valid root seal cannot bless evidence that changed afterward or whose
# immutable inode shape was replaced. All four failures occur before a POST.
for proof_mutation in missing hardlink tamper; do
  case "$proof_mutation" in
    missing) proof_mutation_id="$(printf 'd%.0s' {1..64})" ;;
    hardlink) proof_mutation_id="$(printf 'f%.0s' {1..64})" ;;
    tamper) proof_mutation_id="$(printf '9%.0s' {1..64})" ;;
  esac
  make_proof_tamper_variant "$proof_mutation_id" "$proof_mutation"
  proof_mutation_offset="$(stat -c '%s' "$forge_log")"
  if sudo -n "$publisher" "$request_root/$proof_mutation_id" \
    >"$tmp/proof-$proof_mutation.log" 2>&1; then
    printf 'publisher accepted %s proof evidence\n' "$proof_mutation" >&2
    exit 1
  fi
  [[ "$(stat -c '%s' "$forge_log")" == "$proof_mutation_offset" ]] || {
    printf '%s proof evidence reached the forge\n' "$proof_mutation" >&2
    exit 1
  }
done

# Production uses retain_requests=false. Exercise a complete successful broker
# path with that exact value and prove the consumed root request is removed.
retained_request_count_before="$(
  sudo -n find "$request_root" -mindepth 1 -maxdepth 1 -type d | wc -l
)"
jq '.retain_requests = false' "$valid_sandbox_config" \
  >"$tmp/nonretaining-sandbox-config.json"
sudo -n install -o root -g root -m 0600 \
  "$tmp/nonretaining-sandbox-config.json" "$sandbox_config"
# Earlier publication state-machine cases intentionally leave hostile fake-
# forge readback state behind; this independent success starts from clean OK.
printf '' >"$forge_state"
printf '%s\n' ok >"$forge_behavior"
if ! run_fixture_lane "$tmp/nonretaining-success.log"; then
  cat "$tmp/nonretaining-success.log" >&2
  printf 'production false-retention broker run failed\n' >&2
  exit 1
fi
retained_request_count_after="$(
  sudo -n find "$request_root" -mindepth 1 -maxdepth 1 -type d | wc -l
)"
[[ "$retained_request_count_after" == "$retained_request_count_before" ]] || {
  printf 'false-retention broker left a root request behind\n' >&2
  exit 1
}
sudo -n install -o root -g root -m 0600 \
  "$valid_sandbox_config" "$sandbox_config"

printf 'host CI privilege-separated publication and adversarial isolation contract ok\n'
