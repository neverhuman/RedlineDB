#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tmp="$(mktemp -d /tmp/jain-split-host-integrity-test.XXXXXX)"
forged_root=""
runner_pid=""
forge_pid=""
publisher_root="$tmp/root-publisher"
request_root="$tmp/root-requests"
worker_cache="$tmp/worker-cache"
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
  case "$forged_root" in
    /tmp/split-host-ci-bootstrap.??????) rm -rf -- "$forged_root" ;;
  esac
  sudo -n rm -rf -- "$publisher_root" 2>/dev/null || true
  sudo -n rm -rf -- "$request_root" 2>/dev/null || true
  sudo -n rm -rf -- "$worker_cache" 2>/dev/null || true
  rm -rf -- "$tmp"
  return "$cleanup_rc"
}
trap cleanup EXIT

control="$tmp/control"
control_remote="$tmp/jain-split-ops.git"
split_root="$tmp/split"
sandbox_family_root=/home/ubuntu/jain-split
product="$split_root/jain-report"
product_remote="$product_forge_root/jeryu/jain-report.git"
forge_log="$tmp/forge.log"
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
"$tmp/fake-forge" "$forge_address_file" "$forge_log" >"$tmp/forge.stderr" 2>&1 &
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
touch "$forge_log"

git clone --quiet --shared "$repo_root" "$control"
git -C "$control" config user.name 'Host CI Integration Fixture'
git -C "$control" config user.email host-ci-integration@example.invalid
for boundary_file in \
  ops/ci/host-ci-integrity.sh ops/ci/host-ci-publisher.sh \
  ops/ci/host-ci-sandbox.sh ops/ci/host-ci-boundary-preflight.sh \
  ops/ci/native-runtime.sh \
  ops/ci/split-host-ci-parent.sh ops/ci/split-host-ci.sh; do
  install -D -m 0755 "$repo_root/$boundary_file" "$control/$boundary_file"
done
git init --quiet --bare "$control_remote"
sed -i \
  "s#remote = \"http://127.0.0.1:8787/git/jeryu/jain-split-ops.git\"#remote = \"$control_remote\"#" \
  "$control/repos.manifest.toml"
git -C "$control" add repos.manifest.toml ops/ci
git -C "$control" commit --quiet -m 'fixture reviewed host-CI boundary'
git -C "$control" switch -C main --quiet
git -C "$control" remote set-url origin "$control_remote"
git -C "$control" push --quiet -u origin main
control_commit="$(git -C "$control" rev-parse HEAD)"

# Install the reviewed broker and its credential exactly as production does:
# executable root-only broker, adjacent root-only config. The server itself
# never receives the token through argv or environment.
publisher="$publisher_root/host-ci-publisher"
publisher_config="$publisher_root/host-ci-publisher.config.json"
sandbox="$publisher_root/host-ci-sandbox"
sandbox_config="$publisher_root/host-ci-sandbox.config.json"
sudo -n install -d -o root -g root -m 0711 "$publisher_root"
sudo -n install -d -o root -g root -m 0700 "$request_root"
mkdir -p "$(dirname "$product_remote")"
sudo -n install -o root -g root -m 0500 \
  "$control/ops/ci/host-ci-publisher.sh" "$publisher"
sudo -n install -o root -g root -m 0500 \
  "$control/ops/ci/host-ci-sandbox.sh" "$sandbox"
sudo -n install -d -o xbwork -g xbwork -m 0700 "$worker_cache"
publisher_token="$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')"
jq -cn --arg digest "$(sha256sum "$control/ops/ci/host-ci-publisher.sh" | cut -d' ' -f1)" \
  --arg sandbox_digest "$(sha256sum "$control/ops/ci/host-ci-sandbox.sh" | cut -d' ' -f1)" \
  --arg base "$forge_base" --arg token "$publisher_token" \
  --arg git_base "$product_forge_root" \
  --arg remote "$control_remote" --arg requests "$request_root" \
  '{schema_version:"jain.host-ci-publisher-config/v2",
    publisher_sha256:$digest,sandbox_sha256:$sandbox_digest,
    forge_base:$base,forge_git_base:$git_base,
    control_remote:$remote,request_root:$requests,
    max_seal_age_seconds:300,token:$token}' \
  | sudo -n tee "$publisher_config" >/dev/null
sudo -n chown root:root "$publisher_config"
sudo -n chmod 0600 "$publisher_config"
jq -cn --arg digest "$(sha256sum "$control/ops/ci/host-ci-sandbox.sh" | cut -d' ' -f1)" \
  --arg publisher_digest "$(sha256sum "$control/ops/ci/host-ci-publisher.sh" | cut -d' ' -f1)" \
  --arg family "$sandbox_family_root" --arg cache "$worker_cache" \
  --arg cargo_bin "$HOME/.cargo/bin" --arg rustup "$HOME/.rustup" \
  --arg git_base "$product_forge_root" \
  --arg remote "$control_remote" --arg requests "$request_root" \
  --argjson parent_uid "$(id -u)" --argjson parent_gid "$(id -g)" \
  '{schema_version:"jain.host-ci-sandbox-config/v2",
    sandbox_sha256:$digest,publisher_sha256:$publisher_digest,
    parent_uid:$parent_uid,parent_gid:$parent_gid,
    worker_user:"xbwork",worker_group:"xbwork",family_root:$family,
    worker_cache:$cache,cargo_bin:$cargo_bin,rustup_home:$rustup,
    control_remote:$remote,forge_git_base:$git_base,
    request_root:$requests,retain_requests:true,
    device_allow:[]}' | sudo -n tee "$sandbox_config" >/dev/null
sudo -n chown root:root "$sandbox_config"
sudo -n chmod 0600 "$sandbox_config"
unset publisher_token

mkdir -p "$product/scripts" "$split_root/jain-core"
cat >"$product/scripts/ci-local.sh" <<'SCRIPT'
#!/usr/bin/env bash
set -euo pipefail
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
  recovered=''
  visible_pids=0
  for process in /proc/[0-9]*; do
    [[ -d "$process" ]] || continue
    visible_pids=$((visible_pids + 1))
    for candidate in "$process/environ" "$process/cmdline" "$process"/fd/*; do
      [[ -r "$candidate" ]] || continue
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
      jeryu jain-report "$product_sha" "$tmp/not-a-repository" \
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
forged_root="$(mktemp -d /tmp/split-host-ci-bootstrap.XXXXXX)"
chmod 0700 "$forged_root"
git -C "$control" worktree add --quiet --detach \
  "$forged_root/control-plane" "$control_commit"
git -C "$control" show "$control_commit:ops/ci/split-host-ci.sh" \
  >"$forged_root/.split-host-ci-reviewed"
chmod 0500 "$forged_root/.split-host-ci-reviewed"
forged_seal="$(printf 'ab%.0s' {1..32})"
forged_result="$forged_root/child-result.json"
jq -n --arg seal "$forged_seal" --arg source_root "$control" \
  --arg exact_root "$forged_root/control-plane" \
  --arg commit "$control_commit" --arg result_path "$forged_result" \
  '{schema_version:"jain.host-ci-reexec/v1",seal:$seal,
    source_root:$source_root,exact_root:$exact_root,commit:$commit,
    result_path:$result_path}' \
  >"$forged_root/reexec-state.json"
chmod 0600 "$forged_root/reexec-state.json"
forged_started="$tmp/forged-started"
forged_continue="$tmp/forged-continue"
touch "$forged_continue"
forged_splitctl="$forged_root/splitctl"
cargo build --locked --quiet --manifest-path "$control/Cargo.toml" \
  --bin splitctl --target-dir "$tmp/direct-control-target"
install -m 0500 "$tmp/direct-control-target/debug/splitctl" "$forged_splitctl"
jq --arg splitctl_path "$forged_splitctl" \
  --arg splitctl_sha256 "$(sha256sum "$forged_splitctl" | cut -d' ' -f1)" \
  '. + {splitctl_path:$splitctl_path,splitctl_sha256:$splitctl_sha256}' \
  "$forged_root/reexec-state.json" >"$forged_root/reexec-state.json.tmp"
mv "$forged_root/reexec-state.json.tmp" "$forged_root/reexec-state.json"
chmod 0600 "$forged_root/reexec-state.json"
forge_lines_before="$(wc -l <"$forge_log" 2>/dev/null || echo 0)"
if JAIN_HOST_CI_REEXEC_STATE="$forged_root/reexec-state.json" \
  JAIN_HOST_CI_REEXEC_SEAL="$forged_seal" \
  JAIN_SPLIT_ROOT="$split_root" \
  CARGO_TARGET_DIR="$repo_root/target" \
    bash "$forged_root/.split-host-ci-reviewed" \
      jeryu jain-report "$product_sha" "$product" jain-report/required \
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
git -C "$control" worktree remove --force \
  "$forged_root/control-plane" >/dev/null
rm -rf -- "$forged_root"
forged_root=""

# A commit that exists only in the caller checkout is rejected before any
# candidate starts or any forge publication occurs.
unadvertised_offset="$(stat -c '%s' "$forge_log")"
if JAIN_HOST_CI_SANDBOX="$sandbox" \
  JAIN_SPLIT_ROOT="$sandbox_family_root" \
    "$control/ops/ci/split-host-ci.sh" \
      jeryu jain-report "$unadvertised_sha" "$product" \
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
host_pid_namespace="$(readlink /proc/self/ns/pid)"
host_user_namespace="$(readlink /proc/self/ns/user)"
JAIN_HOST_CI_PUBLISHER="$publisher" \
JAIN_HOST_CI_SANDBOX="$sandbox" \
JAIN_SPLIT_ROOT="$sandbox_family_root" \
JAIN_TEST_ATTACK_URL="$forge_base" \
JAIN_TEST_REQUIRE_ISOLATION=1 \
JAIN_TEST_HOST_PID_NAMESPACE="$host_pid_namespace" \
JAIN_TEST_HOST_USER_NAMESPACE="$host_user_namespace" \
JAIN_TEST_ROOT_CONFIG_PATH="$publisher_config" \
JAIN_TEST_ROOT_REQUEST_PATH="$request_root" \
JAIN_TEST_FS_MONITOR_PATH=/opt/jain-ci/cargo-home/fsmonitor-attack.sh \
  "$control/ops/ci/split-host-ci.sh" \
    jeryu jain-report "$product_sha" "$product" jain-report/required \
    >"$success_log" 2>&1 || {
  cat "$success_log" >&2
  printf 'validated parent could not publish success\n' >&2
  exit 1
}
grep -Fq 'body={"name":"jain-report/required"' "$forge_log" \
  && grep -Fq '"conclusion":"success"' "$forge_log" || {
  cat "$forge_log" >&2
  printf 'publisher did not publish a success check\n' >&2
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
  local nonce result_sha root_seal
  sudo -n cp -a -- "$success_request" "$destination"
  sudo -n rm -rf -- "$destination/publish.lock"
  sudo -n jq --arg request_id "$request_id" "$result_filter" \
    "$success_request/root-result.json" >"$local_result"
  result_sha="$(sha256sum -- "$local_result" | cut -d' ' -f1)"
  nonce="$(sudo -n jq -er '.nonce' "$success_request/root-state.json")"
  root_seal="$({
    printf '%s\n%s\n%s\n%s\n' "$nonce" "$result_sha" "$sealed_at" "$request_id"
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
  JAIN_TEST_FORCE_FAILURE=1 \
    "$control/ops/ci/split-host-ci.sh" \
      jeryu jain-report "$product_sha" "$product" jain-report/required \
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

printf 'host CI privilege-separated publication and adversarial isolation contract ok\n'
