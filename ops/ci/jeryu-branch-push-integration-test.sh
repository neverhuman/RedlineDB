#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [[ "${1:-}" != --inside ]]; then
  mkdir -p "$repo_root/target/test-tmp"
  outer_tmp="$(mktemp -d "$repo_root/target/test-tmp/jeryu-branch-push.XXXXXX")"
  cleanup_outer() {
    cleanup_rc=$?
    if sudo -n find "$outer_tmp" -type l -print -quit | grep -q .; then
      printf 'refusing to remove fixture root containing a symlink: %s\n' "$outer_tmp" >&2
      return 1
    fi
    sudo -n rm -rf -- "$outer_tmp"
    return "$cleanup_rc"
  }
  trap cleanup_outer EXIT

  cargo build --locked --bin splitctl
  rustc --edition=2021 "$repo_root/ops/ci/fake-git-http.rs" \
    -o "$outer_tmp/fake-git-http"
  sudo -n /usr/bin/unshare --net --pid --fork --kill-child=KILL --mount-proc \
    /bin/bash "$repo_root/ops/ci/jeryu-branch-push-integration-test.sh" \
    --inside "$outer_tmp" "$repo_root/target/debug/splitctl" "$outer_tmp/fake-git-http"
  printf 'jeryu branch-push smart-HTTP integration fixture ok\n'
  exit 0
fi

[[ "$#" == 4 ]] || {
  printf 'invalid inner fixture arguments\n' >&2
  exit 64
}
tmp="$2"
source_splitctl="$3"
source_forge="$4"
/usr/sbin/ip link set lo up

inner="$tmp/inner"
project_root="$inner/projects"
source_repo="$inner/source"
token_file="$inner/credential/material/token"
receipt="$inner/branch-push.json"
server_log="$inner/server.log"
server_stderr="$inner/server.stderr"
command_stdout="$inner/command.stdout"
command_stderr="$inner/command.stderr"
materialize_stdout="$inner/materialize.stdout"
materialize_stderr="$inner/materialize.stderr"
materialized="$inner/materialized"
readback_stdout="$inner/readback.stdout"
readback_stderr="$inner/readback.stderr"
fetch_failure_stdout="$inner/fetch-failure.stdout"
fetch_failure_stderr="$inner/fetch-failure.stderr"
ref_move_stdout="$inner/ref-move.stdout"
ref_move_stderr="$inner/ref-move.stderr"
proc_snapshot="$inner/proc-snapshot.txt"
trace_prefix="$inner/exec-trace"
ready_file="$inner/server-ready"
auth_wait_file="$inner/auth-wait"
continue_file="$inner/continue"
behavior_file="$inner/server-behavior"
splitctl="$inner/splitctl"
fake_forge="$inner/fake-git-http"
server_pid=''
trace_pid=''
stage='initialization'

cleanup_inner() {
  cleanup_rc=$?
  if [[ "$cleanup_rc" != 0 ]]; then
    printf 'fixture failed at stage: %s\n' "$stage" >&2
    [[ ! -f "$command_stderr" ]] || tail -40 "$command_stderr" >&2
    [[ ! -f "$server_stderr" ]] || tail -40 "$server_stderr" >&2
    [[ ! -f "$server_log" ]] || tail -40 "$server_log" >&2
    [[ ! -f "$receipt" ]] || jq -S . "$receipt" >&2
    for trace in "$trace_prefix".*; do
      [[ ! -f "$trace" ]] || tail -20 "$trace" >&2
    done
  fi
  if [[ -n "$trace_pid" ]] && kill -0 "$trace_pid" 2>/dev/null; then
    kill "$trace_pid" 2>/dev/null || true
    wait "$trace_pid" 2>/dev/null || true
  fi
  if [[ -n "$server_pid" ]] && kill -0 "$server_pid" 2>/dev/null; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  return "$cleanup_rc"
}
trap cleanup_inner EXIT

mkdir -p "$project_root/jeryu" "$source_repo" "$(dirname "$token_file")"
stage='fixture setup'
install -m 0755 "$source_splitctl" "$splitctl"
install -m 0755 "$source_forge" "$fake_forge"

umask 077
token="synthetic-$RANDOM-$RANDOM-$RANDOM-no-authority"
printf '%s\n' "$token" >"$token_file"
chmod 0600 "$token_file"

/usr/bin/git init --bare --quiet "$project_root/jeryu/example.git"
/usr/bin/git --git-dir="$project_root/jeryu/example.git" config http.receivepack true
/usr/bin/git init --quiet -b codex/jeryu-smart-http-integration "$source_repo"
/usr/bin/git -C "$source_repo" config user.name fixture
/usr/bin/git -C "$source_repo" config user.email fixture@localhost
printf 'fixture\n' >"$source_repo/fixture.txt"
/usr/bin/git -C "$source_repo" add fixture.txt
/usr/bin/git -C "$source_repo" commit --quiet -m fixture
head_sha="$(/usr/bin/git -C "$source_repo" rev-parse HEAD)"

"$fake_forge" "$project_root" "$token_file" "$server_log" "$ready_file" \
  "$auth_wait_file" "$continue_file" "$behavior_file" \
  >"$inner/server.stdout" 2>"$server_stderr" &
server_pid=$!
for _ in $(seq 1 500); do
  [[ -s "$ready_file" ]] && break
  kill -0 "$server_pid" 2>/dev/null || break
  sleep 0.01
done
[[ -s "$ready_file" ]]

stage='authenticated branch publication'
/usr/bin/strace -ff -s 512 -o "$trace_prefix" -e trace=execve \
  "$splitctl" jeryu-local branch-push \
  --repo jeryu/example \
  --repo-path "$source_repo" \
  --branch codex/jeryu-smart-http-integration \
  --expected-head "$head_sha" \
  --token-file "$token_file" \
  --evidence-out "$receipt" \
  --apply >"$command_stdout" 2>"$command_stderr" &
trace_pid=$!

for _ in $(seq 1 1500); do
  [[ -s "$auth_wait_file" ]] && break
  kill -0 "$trace_pid" 2>/dev/null || break
  sleep 0.01
done
[[ -s "$auth_wait_file" ]]

stage='held-child process inspection'
descendants=("$trace_pid")
for ((index = 0; index < ${#descendants[@]}; index++)); do
  pid="${descendants[$index]}"
  [[ -r "/proc/$pid/task/$pid/children" ]] || continue
  read -r -a children <"/proc/$pid/task/$pid/children" || true
  descendants+=("${children[@]}")
done

token_identity="$(stat -Lc '%d:%i' "$token_file")"
splitctl_identity="$(stat -Lc '%d:%i' "$splitctl")"
observed_splitctl=false
observed_git=false
observed_remote_http=false
observed_pinned_askpass_fd=false
: >"$proc_snapshot"
for pid in "${descendants[@]}"; do
  [[ -d "/proc/$pid" ]] || continue
  comm="$(<"/proc/$pid/comm")"
  case "$comm" in
    splitctl) observed_splitctl=true ;;
    git) observed_git=true ;;
    git-remote-http) observed_remote_http=true ;;
  esac
  printf 'pid=%s comm=%s\n' "$pid" "$comm" >>"$proc_snapshot"
  tr '\0' '\n' <"/proc/$pid/cmdline" >>"$proc_snapshot"
  tr '\0' '\n' <"/proc/$pid/environ" >>"$proc_snapshot"
  for fd in /proc/"$pid"/fd/*; do
    [[ -e "$fd" ]] || continue
    fd_target="$(readlink "$fd")"
    printf 'fd=%s target=%s\n' "${fd##*/}" "$fd_target" >>"$proc_snapshot"
    if [[ "$comm" == git || "$comm" == git-remote-http ]]; then
      fd_identity="$(stat -Lc '%d:%i' "$fd")"
      [[ "$fd_identity" != "$token_identity" ]] || {
        printf 'Git child inherited the token-file descriptor\n' >&2
        exit 1
      }
      if [[ "$fd_identity" == "$splitctl_identity" ]]; then
        observed_pinned_askpass_fd=true
      fi
    fi
  done
done
[[ "$observed_splitctl" == true && "$observed_git" == true && "$observed_remote_http" == true ]]
[[ "$observed_pinned_askpass_fd" == true ]]

if printf '%s\n' "$token" | grep -F -f - "$proc_snapshot" >/dev/null; then
  printf 'synthetic token leaked through child /proc state\n' >&2
  exit 1
fi

: >"$continue_file"
stage='branch publication completion'
wait "$trace_pid"
trace_pid=''

stage='authenticated exact-head materialization'
"$splitctl" jeryu-local git-materialize \
  --repo jeryu/example \
  --remote http://127.0.0.1:8787/git/jeryu/example.git \
  --ref refs/heads/codex/jeryu-smart-http-integration \
  --expected-head "$head_sha" \
  --destination "$materialized" \
  --token-file "$token_file" >"$materialize_stdout" 2>"$materialize_stderr"
[[ "$(/usr/bin/git -C "$materialized" rev-parse 'HEAD^{commit}')" == "$head_sha" ]]
[[ -z "$(/usr/bin/git -C "$materialized" remote)" ]]
[[ -z "$(/usr/bin/git -C "$materialized" status --porcelain=v1 \
  --untracked-files=all)" ]]

stage='authenticated exact-head ref readback'
"$splitctl" jeryu-local ref-readback \
  --repo jeryu/example \
  --remote http://127.0.0.1:8787/git/jeryu/example.git \
  --ref refs/heads/codex/jeryu-smart-http-integration \
  --expected-head "$head_sha" \
  --token-file "$token_file" >"$readback_stdout" 2>"$readback_stderr"
jq -e --arg head "$head_sha" '
  .schema_version == "jain.jeryu-ref-readback/v1" and
  .status == "pass" and
  .expected_head == $head and
  .advertised_refs == ["refs/heads/codex/jeryu-smart-http-integration"]
' "$readback_stdout" >/dev/null
if "$splitctl" jeryu-local ref-readback \
  --repo jeryu/example \
  --remote http://127.0.0.1:8787/git/jeryu/example.git \
  --expected-head 0000000000000000000000000000000000000000 \
  --token-file "$token_file" >/dev/null 2>"$readback_stderr"; then
  printf 'authenticated ref readback accepted an unadvertised head\n' >&2
  exit 1
fi
grep -Fq 'requested head is not an advertised product ref' "$readback_stderr"

stage='authenticated fetch failure cleanup'
printf 'fail-upload-pack\n' >"$behavior_file"
fetch_failure_destination="$inner/fetch-failure-materialized"
if "$splitctl" jeryu-local git-materialize \
  --repo jeryu/example \
  --remote http://127.0.0.1:8787/git/jeryu/example.git \
  --ref refs/heads/codex/jeryu-smart-http-integration \
  --expected-head "$head_sha" \
  --destination "$fetch_failure_destination" \
  --token-file "$token_file" \
  >"$fetch_failure_stdout" 2>"$fetch_failure_stderr"; then
  printf 'materializer accepted an authenticated upload-pack failure\n' >&2
  exit 1
fi
[[ ! -e "$fetch_failure_destination" && ! -L "$fetch_failure_destination" ]]

stage='advertised ref move rejection'
printf 'moved fixture\n' >"$source_repo/fixture.txt"
/usr/bin/git -C "$source_repo" add fixture.txt
/usr/bin/git -C "$source_repo" commit --quiet -m 'moved fixture ref'
moved_sha="$(/usr/bin/git -C "$source_repo" rev-parse HEAD)"
/usr/bin/git -C "$source_repo" push --quiet \
  "$project_root/jeryu/example.git" "$moved_sha:refs/heads/moved-target"
printf 'move-ref %s %s %s\n' \
  refs/heads/codex/jeryu-smart-http-integration \
  "$moved_sha" "$head_sha" >"$behavior_file"
ref_move_destination="$inner/ref-move-materialized"
if "$splitctl" jeryu-local git-materialize \
  --repo jeryu/example \
  --remote http://127.0.0.1:8787/git/jeryu/example.git \
  --ref refs/heads/codex/jeryu-smart-http-integration \
  --expected-head "$head_sha" \
  --destination "$ref_move_destination" \
  --token-file "$token_file" >"$ref_move_stdout" 2>"$ref_move_stderr"; then
  printf 'materializer accepted a ref move during fetch\n' >&2
  exit 1
fi
[[ ! -e "$ref_move_destination" && ! -L "$ref_move_destination" ]]
[[ "$(/usr/bin/git --git-dir="$project_root/jeryu/example.git" \
  rev-parse refs/heads/codex/jeryu-smart-http-integration)" == "$moved_sha" ]]
/usr/bin/git --git-dir="$project_root/jeryu/example.git" update-ref \
  refs/heads/codex/jeryu-smart-http-integration "$head_sha" "$moved_sha"

kill "$server_pid"
wait "$server_pid" 2>/dev/null || true
server_pid=''

remote_head="$(/usr/bin/git --git-dir="$project_root/jeryu/example.git" \
  rev-parse refs/heads/codex/jeryu-smart-http-integration)"
[[ "$remote_head" == "$head_sha" ]]

stage='receipt and non-disclosure validation'
jq -e --arg head "$head_sha" '
  .schema_version == "jain.jeryu-branch-publication/v1" and
  .status == "pass" and
  .mode == "apply" and
  .external_state_changed == true and
  .before == null and
  .after == $head and
  .expected_head == $head and
  .remote == "http://127.0.0.1:8787/git/jeryu/example.git" and
  .push_result == "authenticated-non-force-push-completed" and
  .action == "pushed-and-verified"
' "$receipt" >/dev/null
sha256sum "$receipt" >"$receipt.sha256"
sha256sum --check "$receipt.sha256" >/dev/null

grep -F 'execve("/proc/self/fd/' "$trace_prefix".* >/dev/null
grep -F "Password for 'http://x-access-token@127.0.0.1:8787': " \
  "$trace_prefix".* >/dev/null
grep -F 'auth=absent' "$server_log" >/dev/null
grep -F 'auth=valid' "$server_log" >/dev/null

for output in "$command_stdout" "$command_stderr" "$server_log" "$server_stderr" \
  "$materialize_stdout" "$materialize_stderr" \
  "$fetch_failure_stdout" "$fetch_failure_stderr" \
  "$ref_move_stdout" "$ref_move_stderr" \
  "$receipt" "$receipt.sha256" "$proc_snapshot" "$trace_prefix".*; do
  if printf '%s\n' "$token" | grep -F -f - "$output" >/dev/null; then
    printf 'synthetic token leaked to fixture output: %s\n' "$output" >&2
    exit 1
  fi
done

trap - EXIT
cleanup_inner
