#!/usr/bin/env bash
# Reconfigure the bench-native shell runner to run as the host user
# `ubuntu` (where rustup + cargo are installed) instead of the default
# `gitlab-runner` system user.
#
# The default `gitlab-runner` user can't see /home/ubuntu/.cargo/bin
# (permissions) and the per-runner `--env "PATH=..."` flag we set during
# registration isn't applied early enough for `command -v cargo` to
# succeed during step_script. Running AS the ubuntu user fixes both
# (user owns /home/ubuntu, has /home/ubuntu/.cargo/bin in PATH by
# default via ~/.bashrc).
#
# Re-runnable. Idempotent.

set -euo pipefail

if [ "$(id -u)" != "0" ]; then
    printf 'Run as root (sudo).\n' >&2
    exit 1
fi

RUNNER_USER="${RUNNER_USER:-ubuntu}"
log() { printf '[fix-bench-user] %s\n' "$*" >&2; }

# 1. Reinstall gitlab-runner service with --user ubuntu
log "Stopping and reinstalling gitlab-runner systemd unit to run as ${RUNNER_USER} ..."
systemctl stop gitlab-runner.service 2>&1 | tail -2 || true
gitlab-runner uninstall 2>&1 | tail -2 || true
gitlab-runner install \
    --user "$RUNNER_USER" \
    --working-directory "/home/${RUNNER_USER}/.gitlab-runner-data"

# 2. Make sure builds_dir and cache_dir are owned by ubuntu
mkdir -p "/home/${RUNNER_USER}/.gitlab-runner-data" \
    /var/lib/gitlab-runner/bench-native-builds \
    /var/lib/gitlab-runner/bench-native-cache
chown -R "$RUNNER_USER:$RUNNER_USER" \
    "/home/${RUNNER_USER}/.gitlab-runner-data" \
    /var/lib/gitlab-runner/bench-native-builds \
    /var/lib/gitlab-runner/bench-native-cache

# 3. Start the service
systemctl daemon-reload
systemctl enable gitlab-runner.service 2>&1 | tail -2 || true
systemctl start gitlab-runner.service
sleep 3

# 4. Verify
systemctl is-active gitlab-runner.service >/dev/null && log "Service active." || {
    log "ERROR: service failed to start. Check journalctl -u gitlab-runner."
    exit 1
}

# 5. Smoke test: bench-native should now see cargo
sudo -u "$RUNNER_USER" bash -c 'echo "ubuntu user PATH=$PATH"; command -v cargo && cargo --version' 2>&1 | head -5

cat <<EOF

✓ Done.

The gitlab-runner service now runs as user '${RUNNER_USER}'. The
bench-native shell runner will inherit:
  - /home/${RUNNER_USER}/.cargo/bin on PATH (cargo, cargo-nextest, etc)
  - /home/${RUNNER_USER}/.rustup (rust toolchain)
  - file ownership of /home/${RUNNER_USER}/ for git checkouts

Trigger any push to main and watch the bench-native jobs succeed.
EOF
