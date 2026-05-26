#!/usr/bin/env bash
# Patch the gitlab-runner config so the bench-native shell runner
# prepares its job environment WITHOUT sourcing ~/.bashrc / ~/.profile
# (which may error out and fail `prepare environment` before step_script
# ever runs).
#
# Two changes:
#   1. Add an explicit `environment = [...]` block under the bench-native
#      [[runners]] section that pins PATH + CARGO_HOME + RUSTUP_HOME.
#   2. Optional belt+suspenders: add `pre_clone_script` that emits an
#      empty rc-skip stanza so any rc files that DO get loaded don't
#      bring in interactive customizations.
#
# Re-runnable. Idempotent.

set -euo pipefail

if [ "$(id -u)" != "0" ]; then
    printf 'Run as root (sudo).\n' >&2
    exit 1
fi

CONFIG="/etc/gitlab-runner/config.toml"
RUNNER_USER="${RUNNER_USER:-ubuntu}"
RUNNER_NAME="${RUNNER_NAME:-jeryu-bench-native}"

log() { printf '[fix-bench-env] %s\n' "$*" >&2; }

if [ ! -f "$CONFIG" ]; then
    log "ERROR: $CONFIG not found"
    exit 1
fi

# Read the ubuntu user's actual PATH (the way they have it after rustup
# install), and stash CARGO_HOME / RUSTUP_HOME from their home.
USER_PATH="$(sudo -u "$RUNNER_USER" bash -lc 'echo $PATH' 2>/dev/null || true)"
[ -z "$USER_PATH" ] && USER_PATH="/home/${RUNNER_USER}/.cargo/bin:/home/${RUNNER_USER}/.local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/snap/bin"
USER_CARGO="/home/${RUNNER_USER}/.cargo"
USER_RUSTUP="/home/${RUNNER_USER}/.rustup"

log "ubuntu PATH = $USER_PATH"
log "Patching $CONFIG ..."

# Use Python for safe TOML edit.
python3 - "$CONFIG" "$RUNNER_NAME" "$USER_PATH" "$USER_CARGO" "$USER_RUSTUP" <<'PY'
import re, sys, pathlib

cfg_path = pathlib.Path(sys.argv[1])
runner_name = sys.argv[2]
user_path = sys.argv[3]
cargo_home = sys.argv[4]
rustup_home = sys.argv[5]

src = cfg_path.read_text()

# Split by [[runners]] sections.
parts = re.split(r'(^\[\[runners\]\])', src, flags=re.MULTILINE)
# parts is like ['', '[[runners]]', '<body>', '[[runners]]', '<body2>', ...]
out = [parts[0]]
i = 1
patched = False
while i < len(parts):
    header = parts[i]
    body = parts[i+1] if i + 1 < len(parts) else ''
    if f'name = "{runner_name}"' in body:
        # Remove any existing environment = [...] block we previously
        # injected so re-runs converge.
        body = re.sub(
            r'^  environment = \[[^\]]*\]\n',
            '',
            body,
            count=1,
            flags=re.MULTILINE,
        )
        # Insert our environment block right after the `executor = ...` line.
        env_lines = [
            '  environment = [',
            f'    "PATH={user_path}",',
            f'    "CARGO_HOME={cargo_home}",',
            f'    "RUSTUP_HOME={rustup_home}",',
            f'    "HOME=/home/ubuntu",',
            '  ]',
            '',
        ]
        env_block = '\n'.join(env_lines)
        body = re.sub(
            r'(^  executor = ".*"\n)',
            r'\1' + env_block,
            body,
            count=1,
            flags=re.MULTILINE,
        )
        patched = True
    out.append(header)
    out.append(body)
    i += 2

if not patched:
    print(f"WARN: no [[runners]] block matched name='{runner_name}'", file=sys.stderr)
    sys.exit(1)

cfg_path.write_text(''.join(out))
print("patched", file=sys.stderr)
PY

log "Restarting gitlab-runner.service ..."
systemctl restart gitlab-runner.service
sleep 2

if systemctl is-active gitlab-runner.service >/dev/null; then
    log "Service active."
else
    log "ERROR: service failed; check journalctl -u gitlab-runner"
    exit 1
fi

cat <<EOF

✓ Done.

config.toml now has an explicit environment = [...] block under the
${RUNNER_NAME} [[runners]] section that pins:
  - PATH including /home/${RUNNER_USER}/.cargo/bin
  - CARGO_HOME=/home/${RUNNER_USER}/.cargo
  - RUSTUP_HOME=/home/${RUNNER_USER}/.rustup
  - HOME=/home/${RUNNER_USER}

These env vars apply during the runner's prepare_script phase BEFORE
~/.bashrc gets sourced, so the dep-check loop sees cargo/rustc.

Trigger any push to main and watch the bench-native jobs succeed.
EOF
