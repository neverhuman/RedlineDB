# Jain host-runner security blocker fix design — 2026-07-16

Status: **DESIGN PASS / APPLY BLOCKED ON STOPPED-HEAD HANDOFF**  
Claim: `codex-runner-rustsec-credential-design-20260716T0538Z`  
Scope: read-only design; no source, CI, credential, PR, forge, or worktree mutation.

## Exact baseline and observed failures

- Live pushed control-plane branch:
  `claude/v8-release-fixes-20260714` at
  `6dcfe2a027621d742d96e7f7c9b80b63511e97bf`.
- The held canonical checkout is not the patch baseline: local `HEAD` is
  `90d412706eb385d247c972812f8775e5c57e4488`, its live branch ref is still
  `6dcfe2a0`, and it contains unrelated dirty release evidence. The sole committed
  source delta between those commits is a six-line `safe.directory = "*"` addition
  in `ops/ci/split-host-ci.sh`; an implementer must preserve that addition after a
  stopped-head handoff and rebase this design onto the then-current pushed head.
- Pushed runner source `ops/ci/split-host-ci.sh`:
  - lines 51–64 interpolate the forge credential into two curl `-H` arguments;
    the credential is therefore observable in curl's process argv;
  - lines 231–249 replace `HOME` with an empty sandbox directory and do not seed
    or export `JAIN_ADVISORY_DB`;
  - release security then resolves the default database as
    `$HOME/.cargo/advisory-db`, which is absent.
- Exact Jain LLM failure evidence:
  `docs/release-evidence/8.0.1/ci/host/380d2d4bb725ae3355aade3148965d07bb506009f8f7b01db9e5d5eccdf5faa3.log`.
  All exercised code/test gates, release Cargo commands, gitleaks, actionlint,
  and cargo-deny passed. The first failing boundary was the missing isolated
  RustSec database.
- Current operator RustSec source is a standalone, non-symlink Git checkout at
  `/home/ubuntu/.cargo/advisory-db`, clean with zero symlinks at:
  - commit `9f3e138091487e69144f536d36976e427a7a3307`;
  - tree `c33f1047906505cabcec7e21f2d99db5c6de8852`.
- Host curl is 8.5.0 and supports reading headers from stdin with
  `--header @-`. The existing Jeryu-family runner already establishes the same
  non-argv header-file precedent with curl `-H @<file>`.
- Existing control-plane tests do not exercise either behavior. The closest
  test is `tests/safety_properties.rs::active_control_plane_ci_has_zero_worktree_and_linked_sibling_policy`,
  which statically protects the no-worktree/no-symlink rule only.

## Minimal patch set

Exactly four files should change. Do not refactor adjacent runner behavior.

1. Add `ops/ci/rustsec-advisory-db.commit` containing one line:

   ```text
   9f3e138091487e69144f536d36976e427a7a3307
   ```

   This is the reviewed immutable pin. Updating it later requires a separate
   reviewed security update; an environment variable may select a source path,
   but may not override this commit.

2. Add sourceable `ops/ci/host-ci-security.sh`. It must not change shell options
   because the caller intentionally uses `set -uo pipefail` rather than `set -e`.
   Patch-ready body:

   ```bash
   #!/usr/bin/env bash
   # Security-sensitive helpers sourced by split-host-ci.sh.

   jeryu_token() {
     if [ -n "${STATUS_TOKEN:-}" ]; then
       printf '%s' "$STATUS_TOKEN"
       return
     fi
     if [ -n "${JERYU_MERGE_TOKEN:-}" ]; then
       printf '%s' "$JERYU_MERGE_TOKEN"
       return
     fi
     local token_file="${JERYU_MERGE_TOKEN_FILE:-$HOME/.jeryu/secrets/merge-token}"
     [ -r "$token_file" ] && tr -d '\n' < "$token_file"
   }

   jeryu_curl() {
     local token rc
     token="$(jeryu_token)"
     [ -n "$token" ] || return 1
     printf 'Authorization: Bearer %s\n' "$token" |
       env -u JERYU_MERGE_TOKEN curl --header @- "$@"
     rc=$?
     token=""
     return "$rc"
   }

   seed_pinned_advisory_db() {
     local source="$1" destination="$2" expected="$3"
     local source_head source_tree

     [[ "$expected" =~ ^[0-9a-f]{40}$ ]] || {
       printf '[split-host-ci] invalid RustSec commit pin\n' >&2
       return 1
     }
     [ -d "$source/.git" ] && [ ! -L "$source" ] && [ ! -L "$source/.git" ] || {
       printf '[split-host-ci] RustSec source is not a standalone Git checkout: %s\n' "$source" >&2
       return 1
     }
     [ -z "$(find "$source" -type l -print -quit 2>/dev/null)" ] || {
       printf '[split-host-ci] RustSec source contains a symlink: %s\n' "$source" >&2
       return 1
     }
     source_head="$(git -C "$source" rev-parse --verify 'HEAD^{commit}')" || return 1
     [ "$source_head" = "$expected" ] || {
       printf '[split-host-ci] RustSec source is not at the reviewed pin\n' >&2
       return 1
     }
     [ -z "$(git -C "$source" status --porcelain=v1 --untracked-files=all)" ] || {
       printf '[split-host-ci] RustSec source is dirty\n' >&2
       return 1
     }
     source_tree="$(git -C "$source" rev-parse --verify 'HEAD^{tree}')" || return 1
     [ ! -e "$destination" ] || return 1

     git clone --quiet --no-local --no-checkout "$source" "$destination" || return 1
     git -C "$destination" checkout --quiet --detach --force "$expected" || return 1
     git -C "$destination" remote remove origin || return 1
     [ "$(git -C "$destination" rev-parse --verify 'HEAD^{commit}')" = "$expected" ] || return 1
     [ "$(git -C "$destination" rev-parse --verify 'HEAD^{tree}')" = "$source_tree" ] || return 1
     [ -z "$(git -C "$destination" status --porcelain=v1 --untracked-files=all)" ] || return 1
     [ -z "$(find "$destination" -type l -print -quit 2>/dev/null)" ] || return 1

     # Close the source-read race: it must still be exact and clean after clone.
     [ "$(git -C "$source" rev-parse --verify 'HEAD^{commit}')" = "$expected" ] || return 1
     [ -z "$(git -C "$source" status --porcelain=v1 --untracked-files=all)" ] || return 1
   }
   ```

3. Modify `ops/ci/split-host-ci.sh` at pushed commit `6dcfe2a0` only as follows.

   - Immediately after `OPS_ROOT=...` (current line 15), source the helper:

     ```bash
     # shellcheck source=ops/ci/host-ci-security.sh
     source "$OPS_ROOT/ops/ci/host-ci-security.sh"
     ```

   - Delete the in-file `jeryu_token` definition at current lines 34–41. Keep
     `STATUS_TOKEN=""`.
   - In `post_check`, change the local declaration to
     `local conclusion="$1" status_state`, remove the local token read/check,
     and replace each `curl ... -H "Authorization: Bearer $token"` with:

     ```bash
     jeryu_curl -fsS -X POST "$JAIN_BASE/repos/$OWNER/$REPO/check-runs" \
       -H 'content-type: application/json' \
       -d "{\"name\":\"$CHECK\",\"head_sha\":\"$SHA\",\"status\":\"completed\",\"conclusion\":\"$conclusion\"}" \
       >/dev/null || return 1
     ```

     Apply the identical `jeryu_curl` substitution to the statuses POST. The
     authorization header is supplied only over curl stdin; argv contains
     `--header @-`, never the credential.
   - Replace current lines 206–208 with capture into a deliberately unexported
     shell variable, then erase any inherited token environment:

     ```bash
     unset STATUS_TOKEN
     STATUS_TOKEN="$(jeryu_token)"
     [ -n "$STATUS_TOKEN" ] || { say "forge status credential is unavailable"; exit 2; }
     unset JERYU_MERGE_TOKEN
     export -n STATUS_TOKEN 2>/dev/null || true
     ```

   - Immediately before replacing `HOME`, capture the operator database source:

     ```bash
     advisory_source="${JAIN_ADVISORY_DB:-$HOME/.cargo/advisory-db}"
     ```

   - Immediately after the sandbox Git config and HOME/CARGO exports (current
     lines 240–249; preserve the held `safe.directory` hunk), seed only release
     runs and repoint the member-repository security lanes:

     ```bash
     if [ "${JAIN_RELEASE_CI:-0}" = "1" ]; then
       advisory_commit="$(tr -d '\r\n' < "$OPS_ROOT/ops/ci/rustsec-advisory-db.commit")"
       sandbox_advisory_db="$HOME/.cargo/advisory-db"
       seed_pinned_advisory_db "$advisory_source" "$sandbox_advisory_db" "$advisory_commit" || {
         say "failed to seed the clean pinned RustSec advisory database"
         exit 2
       }
       export JAIN_ADVISORY_DB="$sandbox_advisory_db"
       printf 'advisory_database_commit=%s\n' "$advisory_commit" >> "$IDENTITY_LOG"
     fi
     ```

   This fails before native-vendor compilation when the source is absent,
   dirty, symlinked, or at the wrong commit. It never copies the operator Cargo
   home, never fetches, and gives every release security lane a sandbox-owned,
   independently cloned, detached, exact-pin database.

4. Add Rust integration tests in `tests/host_ci_security.rs`, using the existing
   `Scratch`/`Drop` style from `tests/safety_properties.rs` and only the standard
   library. Tests must execute the helper through `bash -c 'source ...; ...'`;
   they must not use a real forge, real token, network, or worktree.

   Required tests and exact assertions:

   - `seed_produces_clean_exact_detached_database`
     - create a standalone scratch Git repository with one commit;
     - invoke `seed_pinned_advisory_db(source, destination, exact_commit)`;
     - assert destination `.git` is a directory, not a `.git` file;
     - assert detached `HEAD`, tree and commit equal the source pin;
     - assert porcelain is empty, `origin` is absent, and recursive symlink count
       is zero.
   - `seed_rejects_dirty_or_wrong_pin_before_use`
     - add one untracked source file and assert nonzero exit plus no destination;
     - clean it, pass a different 40-hex commit and assert the same;
     - run once through a symlinked source path and assert the same.
   - `jeryu_curl_keeps_sentinel_out_of_process_argv_and_environment`
     - use a fake scratch `curl` that blocks briefly and records only its own
       `/proc/self/cmdline`, whether `JERYU_MERGE_TOKEN` is present, and stdin;
     - use a synthetic sentinel credential, capture it into unexported
       `STATUS_TOKEN`, unset `JERYU_MERGE_TOKEN`, and invoke `jeryu_curl`;
     - inspect every descendant process cmdline while fake curl is blocked;
     - assert the sentinel occurs in no argv and no child environment;
     - assert fake curl received exactly one stdin authorization header and its
       argv contains `--header` plus `@-`;
     - remove the scratch capture without printing the sentinel.
   - `active_runner_wires_the_security_helpers_and_immutable_pin`
     - read the runner/helper/pin as text;
     - assert the runner sources the helper, seeds before `scripts/ci-local.sh`,
       and exports sandbox `JAIN_ADVISORY_DB`;
     - assert neither runner nor helper contains
       `-H "Authorization: Bearer $token"`;
     - assert the pin is exactly one lowercase 40-hex line.

## Verification sequence after authorization

Run only after the held checkout receives a stopped-head handoff and this patch
is protected on a fresh branch:

1. `bash -n ops/ci/host-ci-security.sh ops/ci/split-host-ci.sh`
2. `shellcheck ops/ci/host-ci-security.sh ops/ci/split-host-ci.sh`
3. `cargo test --locked --test host_ci_security`
4. Existing `cargo test --locked --test safety_properties`
5. Negative fixture runs for absent, dirty, wrong-pin, and symlinked databases;
   each must stop before any forge POST.
6. One exact-head release CI canary with a process observer proving the token is
   absent from all descendant argv/environments and the identity log records
   only the advisory commit, never credential material.
7. Read back both required/check-run objects at the exact canary SHA and prove
   automatic sandbox cleanup.

Success is all of the following: exact pinned DB in isolated HOME; no public
network fetch; dirty/wrong/missing DB fail before build/post; curl receives the
credential only over stdin; credential absent from argv/environment/logs;
existing zero-worktree/standalone-clone rules remain green.

## Credential rotation order

Treat the current merge credential as exposed. Do not rotate it while any old
runner can start, because that would expose the replacement immediately.

1. Stop admission and quiesce every old check publisher. Prove no
   `split-host-ci.sh`, `ci-local.sh`, or forge-write curl process is alive.
   Temporarily bar `ops/onboard.sh` and `ops/split/register-family.sh`; they have
   separate argv-header call sites and must not use the replacement until fixed
   or retired.
2. Protected-land and install this runner fix using the Rust Jeryu client path,
   not an argv-bearing curl invocation. Verify the installed merged script and
   test receipt hashes before allowing a publisher to start.
3. Create a new least-privilege forge-write credential using the owner-approved
   Jeryu token lifecycle. Do not invent a rotation command: first read back the
   live forge's supported create/revoke interface and record only token IDs or
   fingerprints, never values.
4. Write the replacement to a new mode-0600 file in the same directory, fsync
   it, and atomically rename it over the configured token file. Keep admission
   closed. Do not put the value in shell argv, environment, history, receipts,
   chat, or logs.
5. With the patched helper, verify an authenticated bounded read and one
   designated test check. Simultaneously inspect descendant `/proc` argv and
   environment and verify the new credential is absent. Refresh the local Jeryu
   host entry with `jeryu gh-setup --token-file ...` only if that consumer is
   still required.
6. Revoke the old credential immediately after the new one is proven. Verify
   the old token ID is rejected and the new token succeeds; record conclusions
   and fingerprints only.
7. Rotate or revoke any copies on other hosts/consumers, inspect process-monitor
   and CI logs for prior argv capture, then reopen admission. Re-run the exact
   failed heads and read back required/proof; never reuse a green conclusion
   produced before rotation.

If the forge cannot overlap old and new tokens, retain the same quiescence and
perform steps 4–6 as one owner-controlled maintenance transaction; fail closed
with admission still disabled if verification fails.

## Terminal decision

The fix is patch-ready and bounded, but **not applied**. The current canonical
control plane is held, dirty, and ahead of its live branch; applying now would
violate ownership and could overwrite the unpushed `safe.directory` repair.
No credential was read, printed, rotated, or embedded in this receipt.
