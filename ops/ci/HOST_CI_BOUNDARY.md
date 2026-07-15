# Host-CI privilege boundary

`split-host-ci.sh` deliberately fails closed until an administrator provisions
two installed, root-owned brokers and a dedicated non-login worker identity.
The checkout runner never reads a Jeryu credential. The worker cannot reach the
forge, see ancestor processes, see home directories or the installed brokers,
or acquire privilege. A fixed root-owned launcher creates nested PID and user
namespaces, then irreversibly drops all capabilities before reviewed or
candidate-controlled bytes execute. The sandbox fetches a root-owned immutable
control checkout from the configured reviewed remote, derives native-evidence
policy from it, kills the worker cgroup, and seals a one-time result. Only then
does it invoke the root-only publisher. The unprivileged parent has no sudo rule
for the publisher and cannot create, replay, or refresh a root nonce.
The sandbox snapshots the caller request into root-only storage before parsing
it, so a writable descriptor retained across the ownership transition cannot
change the run. It also uses one reviewed, root-owned `splitctl`; callers cannot
select or build the authority binary. Every published required check runs the
full release lane; a caller cannot downgrade it to the quicker merge lane.
The canonical RustSec worktree is read only by a `setpriv` child running as the
configured non-root source owner. That child has empty global/system Git
configuration, lazy promisor fetches disabled, and all Git transports denied;
root imports only its object pack into a new repository with clean config.

Native evidence is written first to a unique per-request `tmpfs` capped at
32 MiB and 64 inodes. The ledger is exactly 16 regular, single-link files,
capped at 8 MiB per file and 16 MiB total. After the worker cgroup is dead, root
verifies the receipt and reviewed-control binding, copies the files as mode
`0400` into the root-only durable store, and makes the attempt directory mode
`0500`. Promotion preserves at least 1 GiB of free space and retains only the
newest eight attempts per repository check. A symlink, special inode, extra
file, size overflow, low-space store, or unexpected non-native evidence turns
the check into failure; none can become publication authority.

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
useradd --system --user-group --home-dir /var/lib/jain-host-ci \
  --shell /usr/sbin/nologin jain-host-ci
install -d -o jain-host-ci -g jain-host-ci -m 0700 \
  /var/cache/jain-host-ci/cargo
install -d -o root -g root -m 0700 /run/jain-host-ci
install -d -o root -g root -m 0700 \
  /var/lib/jain-host-ci/native-evidence
```

Create `/usr/local/libexec/jain/host-ci-sandbox.config.json` as root mode
`0600`. Replace the digests with `sha256sum` output for the installed files:

```json
{
  "schema_version": "jain.host-ci-sandbox-config/v3",
  "sandbox_sha256": "<64 lowercase hex>",
  "publisher_sha256": "<64 lowercase hex>",
  "splitctl_sha256": "<64 lowercase hex>",
  "parent_uid": 1000,
  "parent_gid": 1000,
  "worker_user": "jain-host-ci",
  "worker_group": "jain-host-ci",
  "family_root": "/home/ubuntu/jain-split",
  "worker_cache": "/var/cache/jain-host-ci/cargo",
  "cargo_bin": "/home/ubuntu/.cargo/bin",
  "rustup_home": "/home/ubuntu/.rustup",
  "control_remote": "http://127.0.0.1:8787/git/jeryu/jain-split-ops.git",
  "forge_git_base": "http://127.0.0.1:8787/git",
  "request_root": "/run/jain-host-ci",
  "native_evidence_root": "/var/lib/jain-host-ci/native-evidence",
  "retain_requests": false,
  "device_allow": []
}
```

Create `/usr/local/libexec/jain/host-ci-publisher.config.json` as root mode
`0600`. Supply the token through a root-only editor or stdin; never place it on
a command line or in an environment variable:

```json
{
  "schema_version": "jain.host-ci-publisher-config/v3",
  "publisher_sha256": "<64 lowercase hex>",
  "sandbox_sha256": "<64 lowercase hex>",
  "splitctl_sha256": "<64 lowercase hex>",
  "forge_base": "http://127.0.0.1:8787",
  "forge_git_base": "http://127.0.0.1:8787/git",
  "control_remote": "http://127.0.0.1:8787/git/jeryu/jain-split-ops.git",
  "request_root": "/run/jain-host-ci",
  "native_evidence_root": "/var/lib/jain-host-ci/native-evidence",
  "max_seal_age_seconds": 300,
  "token": "<root-only Jeryu merge token>"
}
```

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

It fails if modes/digests/UIDs differ, the worker has sudo, the parent has any
sudo rule beyond the exact sandbox command above, request/cache ownership differs,
the durable native-evidence directory is missing or uses `/tmp`, or the systemd
namespace/seccomp probe cannot run. GPU release validation is dispatched by SCQ
to registered GPU workers; do not add nonexistent AtomicSoul GPU devices to this
host boundary. Re-run installation and
preflight for every immutable broker revision; never update a digest without
installing and reviewing the matching bytes.
