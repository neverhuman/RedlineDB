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
useradd --system --user-group --home-dir /var/lib/jain-host-ci \
  --shell /usr/sbin/nologin jain-host-ci
install -d -o jain-host-ci -g jain-host-ci -m 0700 \
  /var/cache/jain-host-ci/cargo
install -d -o root -g root -m 0700 /run/jain-host-ci
```

Create `/usr/local/libexec/jain/host-ci-sandbox.config.json` as root mode
`0600`. Replace the digests with `sha256sum` output for the installed files:

```json
{
  "schema_version": "jain.host-ci-sandbox-config/v2",
  "sandbox_sha256": "<64 lowercase hex>",
  "publisher_sha256": "<64 lowercase hex>",
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
  "retain_requests": false,
  "device_allow": [
    "/dev/nvidiactl rw",
    "/dev/nvidia-uvm rw",
    "/dev/nvidia0 rw"
  ]
}
```

Create `/usr/local/libexec/jain/host-ci-publisher.config.json` as root mode
`0600`. Supply the token through a root-only editor or stdin; never place it on
a command line or in an environment variable:

```json
{
  "schema_version": "jain.host-ci-publisher-config/v2",
  "publisher_sha256": "<64 lowercase hex>",
  "sandbox_sha256": "<64 lowercase hex>",
  "forge_base": "http://127.0.0.1:8787",
  "forge_git_base": "http://127.0.0.1:8787/git",
  "control_remote": "http://127.0.0.1:8787/git/jeryu/jain-split-ops.git",
  "request_root": "/run/jain-host-ci",
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
or the systemd namespace/seccomp probe cannot run. Re-run installation and
preflight for every immutable broker revision; never update a digest without
installing and reviewing the matching bytes.
