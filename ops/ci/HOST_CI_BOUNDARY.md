# Host-CI privilege boundary

`split-host-ci.sh` deliberately fails closed until an administrator provisions
two installed, root-owned brokers and a dedicated non-login worker identity.
The checkout runner never reads a Jeryu credential. The worker cannot reach the
forge, see ancestor processes, see home directories or the installed brokers,
or acquire privilege. A fixed root-owned launcher creates nested PID and user
namespaces, then irreversibly drops all capabilities before reviewed or
candidate-controlled bytes execute. The sandbox uses the root-only credential
to fetch a root-owned immutable control checkout from the configured reviewed
remote, derives native-evidence
policy from it, and kills the worker cgroup. It then starts the separately
installed, digest-pinned Jankurai 1.6.11 auditor in a second network-isolated
unit. Jankurai sees a separate clean, read-only checkout at the requested full
SHA and a bounded output `tmpfs`; it never sees the product worker checkout.
Root kills the audit cgroup, validates and promotes the report plus receipt,
and binds the receipt digest into a one-time result seal. Only then does it
invoke the root-only publisher. The unprivileged parent has no sudo rule for
the publisher and cannot create, replay, or refresh a root nonce.
The sandbox snapshots the caller request into root-only storage before parsing
it, so a writable descriptor retained across the ownership transition cannot
change the run. It also uses one reviewed, root-owned `splitctl`; callers cannot
select or build the authority binary. Every published required check runs the
full release lane; a caller cannot downgrade it to the quicker merge lane.
The canonical RustSec worktree is read only by a `setpriv` child running as the
configured non-root source owner. That child has empty global/system Git
configuration, lazy promisor fetches disabled, and all Git transports denied;
root imports only its object pack into a new repository with clean config.

For a repository whose reviewed manifest policy requires Candle CUDA, root
executes only the canonical digest-pinned `/usr/bin/nvidia-smi` with a clean
environment, bounded output, and a hard timeout. It accepts only a complete,
nonempty `index,uuid,compute_cap` inventory with unique device identities and
one homogeneous capability, normalizes values such as `8.6` to `86`, and seals
the full inventory in a request-scoped root-owned record. The worker receives
`CUDA_COMPUTE_CAP` only from that record. Caller injection, missing or malformed
devices, heterogeneous capabilities, detector failure, or disagreement among
the record, worker, native receipt, root result, and publisher fails closed.
This does not grant GPU devices: `device_allow` remains exactly empty.

Native evidence is written first to a unique per-request `tmpfs` capped at
32 MiB and 64 inodes. The ledger is exactly 16 regular, single-link files,
capped at 8 MiB per file and 16 MiB total. After the worker cgroup is dead, root
verifies the receipt and reviewed-control binding, copies the files as mode
`0400` into the root-only durable store, and makes the attempt directory mode
`0500`. Promotion preserves at least 1 GiB of free space and retains only the
newest eight attempts per repository check. A symlink, special inode, extra
file, size overflow, low-space store, or unexpected non-native evidence turns
the check into failure; none can become publication authority.

Jankurai proof output is independently bounded to three regular, single-link
files in a 16 MiB, 32-inode `tmpfs`. The Rust validator binds the repository,
resolves Jankurai's unambiguous Git head to the full commit inside the exact
checkout, and binds clean tracked state before and after audit, report/run/attempt
IDs, governed policy and optional baseline identity, installed auditor version
and digest, score floor and any configured ratchet, conformance, hard findings,
and caps. Root computes a configured score ratchet from the committed compact
baseline instead of passing that historical schema to Jankurai 1.6.11. The
baseline auditor remains provenance; the current executable, report, and policy
identities must agree. Root promotes
only `report.json` and the `jain.jankurai-exact-sha-evidence/v1`
receipt into a distinct root-only durable store. A valid low-score, ratchet,
nonconformance, finding, cap, or product-lane failure becomes durable negative
evidence and can publish only proof failure followed by required failure. Any
identity mismatch, dirty tree, linked inode, extra file, tampering, or auditor
mismatch gains no publication authority.

Publication is strictly ordered: POST `jankurai/proof`, GET the commit's check
runs and verify the exact receipt digest, attempt, and full SHA, POST
`<repo>/required`, GET and verify that exact required result, POST its commit
status, then GET and verify that exact context, description, state, and SHA. A
proof POST failure prevents required publication. After the first proof POST is
attempted, every failure consumes the request; it can never be replayed.

## One-time administrator installation

Review the exact immutable control-plane commit first. From a root shell (not
by running a checkout script through `sudo`), install its reviewed files:

```bash
install -d -o root -g root -m 0755 /usr/local/libexec/jain
install -o root -g root -m 0500 ops/ci/host-ci-sandbox.sh \
  /usr/local/libexec/jain/host-ci-sandbox
install -o root -g root -m 0500 ops/ci/host-ci-publisher.sh \
  /usr/local/libexec/jain/host-ci-publisher
install -o root -g root -m 0500 ops/ci/host-ci-boundary-preflight.sh \
  /usr/local/libexec/jain/host-ci-boundary-preflight
cargo build --locked --release --bin splitctl
install -o root -g root -m 0500 target/release/splitctl \
  /usr/local/libexec/jain/splitctl
test "$(/home/ubuntu/.jeryu/bin/jankurai --version)" = 'jankurai 1.6.11'
# Copy this only from the separately protected jeryu-tool provisioning result.
# The digest is re-derived in that provisioning environment; do not reuse a
# reviewer-local build digest or any binary found through PATH.
governed_jankurai_sha='<64 lowercase hex from governed install receipt>'
test "$(sha256sum /home/ubuntu/.jeryu/bin/jankurai | cut -d' ' -f1)" = \
  "$governed_jankurai_sha"
install -o root -g root -m 0555 /home/ubuntu/.jeryu/bin/jankurai \
  /usr/local/libexec/jain/jankurai
test "$(sha256sum /usr/local/libexec/jain/jankurai | cut -d' ' -f1)" = \
  "$governed_jankurai_sha"
useradd --system --user-group --home-dir /var/lib/jain-host-ci \
  --shell /usr/sbin/nologin jain-host-ci
install -d -o jain-host-ci -g jain-host-ci -m 0700 \
  /var/cache/jain-host-ci/cargo
install -d -o root -g root -m 0555 \
  /var/lib/jain-host-ci/cargo-registry
install -d -o root -g root -m 0700 /var/lib/jain-host-ci/requests
install -d -o root -g root -m 0700 \
  /var/lib/jain-host-ci/native-evidence
install -d -o root -g root -m 0700 \
  /var/lib/jain-host-ci/proof-evidence
```

Create `/usr/local/libexec/jain/host-ci-sandbox.config.json` as root mode
`0600`. Replace the digests with `sha256sum` output for the installed files:

```json
{
  "schema_version": "jain.host-ci-sandbox-config/v7",
  "sandbox_sha256": "<64 lowercase hex>",
  "publisher_sha256": "<64 lowercase hex>",
  "splitctl_sha256": "<64 lowercase hex>",
  "jankurai_sha256": "<64 lowercase hex from governed install receipt>",
  "parent_uid": 1000,
  "parent_gid": 1000,
  "worker_user": "jain-host-ci",
  "worker_group": "jain-host-ci",
  "family_root": "/home/ubuntu/jain-split",
  "worker_cache": "/var/cache/jain-host-ci/cargo",
  "cargo_registry_cache": "/var/lib/jain-host-ci/cargo-registry",
  "cargo_bin": "/home/ubuntu/.cargo/bin",
  "rustup_home": "/home/ubuntu/.rustup",
  "git_lfs_path": "/usr/bin/git-lfs",
  "git_lfs_sha256": "<64 lowercase hex for physical git-lfs 3.4.1>",
  "nvidia_smi_path": "/usr/bin/nvidia-smi",
  "nvidia_smi_sha256": "<sha256sum /usr/bin/nvidia-smi>",
  "control_remote": "http://127.0.0.1:8787/git/veox/jain-split-ops.git",
  "forge_git_base": "http://127.0.0.1:8787/git",
  "request_root": "/var/lib/jain-host-ci/requests",
  "native_evidence_root": "/var/lib/jain-host-ci/native-evidence",
  "proof_evidence_root": "/var/lib/jain-host-ci/proof-evidence",
  "token_file": "/usr/local/libexec/jain/jeryu-merge-token",
  "retain_requests": false,
  "device_allow": []
}
```

Ordinary installed configs omit `control_ref`, `bootstrap_commit`, and
`bootstrap_expires_at`; omission is exactly `refs/heads/main`. The sole
cycle-breaking exception is an owner-authorized exact-head bootstrap. In that
case both configs must carry the same safe `refs/heads/...` value, the exact
lowercase 40-hex commit advertised by that ref, and a decimal-string Unix
expiry no more than two hours ahead. Sandbox, root state, preflight, and
publisher bind all three values; missing, partial, mismatched, expired,
overlong, or moved authority fails closed. Immediately after the protected
fast-forward merge, reinstall the identical merged bytes with all three fields
removed and prove production `main` authority through preflight and a live
readback.

Ordinary product invocations derive `control_plane_commit` from the local
published `refs/remotes/origin/main`, never from the editable checkout HEAD.
An explicit bootstrap invocation additionally sets
`JAIN_HOST_CI_BOOTSTRAP_REF=refs/heads/<published-branch>`; the parent requires
that named checkout and its matching `refs/remotes/origin/...`, while root
still authenticates the configured ref, commit, and expiry independently.

The sandbox validates a canonical root-owned, single-link `/usr/bin/git-lfs`
at the configured digest and exact 3.4.1 build. Only Starforge may request LFS
materialization. Its authenticated pointer objects are fetched before the
networkless worker begins, then transferred into each standalone checkout from
the root materialization with no network. System/global Git configuration and
tracked `.lfsconfig` are disabled; the worker receives only the exact pinned
`filter.lfs.{process,clean,smudge,required}` values.

The v7 sandbox config also requires the canonical root-owned, mode-0755,
single-link `/usr/bin/nvidia-smi` and its exact digest. Root invokes it only for
the four reviewed CUDA release policies. Keep `device_allow` empty; detector
visibility is inventory authority, not permission to expose a device to the
worker.

Create `/usr/local/libexec/jain/jeryu-merge-token` as a canonical root-owned,
single-link regular file at exact mode `0600`. Supply the token through a
root-only editor or stdin; never place it on a command line, in an environment
variable, or inside broker configuration. Both configs name only its path. Then create
`/usr/local/libexec/jain/host-ci-publisher.config.json` as root mode `0600`:

```json
{
  "schema_version": "jain.host-ci-publisher-config/v5",
  "publisher_sha256": "<64 lowercase hex>",
  "sandbox_sha256": "<64 lowercase hex>",
  "splitctl_sha256": "<64 lowercase hex>",
  "jankurai_sha256": "<64 lowercase hex from governed install receipt>",
  "forge_git_base": "http://127.0.0.1:8787/git",
  "control_remote": "http://127.0.0.1:8787/git/veox/jain-split-ops.git",
  "request_root": "/var/lib/jain-host-ci/requests",
  "native_evidence_root": "/var/lib/jain-host-ci/native-evidence",
  "proof_evidence_root": "/var/lib/jain-host-ci/proof-evidence",
  "max_seal_age_seconds": 300,
  "token_file": "/usr/local/libexec/jain/jeryu-merge-token"
}
```

The installed `splitctl` opens every token path component with no-follow
directory descriptors, opens the final file nonblocking/no-follow/close-on-exec,
requires root ownership, exact mode `0600`, one link and stable inode identity,
and talks only to numeric `127.0.0.1:8787` with an exact Host header. After the
protected successor is installed, rotate the credential and prove the former
credential no longer authenticates before using this publisher for release CI.

The Cargo registry cache contains only root-owned mode `0444` crate archives,
sparse-index records, and `config.json` beneath root-owned mode `0555`
directories. The sandbox binds it read-only. For each release run, `splitctl`
parses the exact `Cargo.lock`, checksum-verifies every selected `.crate`, and
copies only those archives and their sparse-index records into a fresh private
Cargo home before Cargo runs with networking forced offline. Git lock entries
are never treated as registry archives: the parser accepts only immutable local
Jeryu identities with an allowlisted owner, safe repository/tag, and exact
40-hex commit, binds them through the complete lock digest, and rejects every
external, branch, revision, unpinned, or malformed Git source. The worker then
resolves those exact commits only through the scoped local bare-mirror rewrite.

The request root must be on an executable filesystem because the sandbox copies
the digest-pinned controller and reviewed runner beneath it before binding that
authority read-only into the worker. Both preflight and sandbox reject a
`noexec` mount. On hosts where `/run` is mounted `noexec`, use the documented
root-only `/var/lib/jain-host-ci/requests`; the request is still one-shot and is
removed after success or controlled failure when `retain_requests=false`.

Remove/revoke any legacy user-readable merge token. Replace broad passwordless
sudo for the parent account with only the argument-validating sandbox:

```sudoers
ubuntu ALL=(root) NOPASSWD: /usr/local/libexec/jain/host-ci-sandbox *
```

The wildcard permits exactly one caller-owned request path; the sandbox rejects
every other argument shape and creates publication state only beneath the
root-owned request directory. Never grant the parent direct publisher access.
Do not allow `env`, `bash`, `bwrap`, `systemd-run`, or an unrestricted command
through sudo.

Run the installed preflight as root:

```bash
/usr/local/libexec/jain/host-ci-boundary-preflight
```

It fails if modes/digests/UIDs differ, Jankurai is not the governed root-owned
1.6.11 binary, the worker has sudo, the parent has any
sudo rule beyond the exact sandbox command above, request/cache ownership differs,
either durable evidence directory is missing, shared, or uses `/tmp`, or the systemd
namespace/seccomp probe cannot run. GPU release validation is dispatched by SCQ
to registered GPU workers; do not add nonexistent AtomicSoul GPU devices to this
host boundary. Re-run installation and
preflight for every immutable broker revision; never update a digest without
installing and reviewing the matching bytes. The two v5 configs must bind the
same freshly provisioned digest from the protected jeryu-tool manifest/install
receipt; an environment-specific review build digest is not portable authority.

These commands are a post-merge authority-owner procedure. A source/PR lane
must not install the broker, migrate the credential, run the unmerged publisher,
or publish product checks.
