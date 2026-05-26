#!/usr/bin/env bash
# Force the bench-native shell runner to use POSIX `sh` instead of `bash`
# for its `prepare_script` stage so user .bashrc / .bash_logout exit
# codes don't fail the job before step_script runs.
#
# Per https://docs.gitlab.com/runner/shells/#shell-profile-loading the
# bash shell sources .bashrc / .bash_logout during prepare. If those
# exit non-zero (rtk wrappers, interactive checks, etc.) gitlab-runner
# reports "prepare environment: exit status 1" and never starts.
#
# POSIX `sh` doesn't source bash rc files. We still use bash for the
# actual job script via the YAML's before_script (multiline `|` blocks)
# which run under bash regardless of the runner's wrapper shell.
#
# Re-runnable. Idempotent.

set -euo pipefail

if [ "$(id -u)" != "0" ]; then
    printf 'Run as root (sudo).\n' >&2
    exit 1
fi

CONFIG="/etc/gitlab-runner/config.toml"
RUNNER_NAME="${RUNNER_NAME:-jeryu-bench-native}"
log() { printf '[fix-bench-shell] %s\n' "$*" >&2; }

if [ ! -f "$CONFIG" ]; then
    log "ERROR: $CONFIG not found"; exit 1
fi

python3 - "$CONFIG" "$RUNNER_NAME" <<'PY'
import re, sys, pathlib
cfg_path = pathlib.Path(sys.argv[1])
runner_name = sys.argv[2]
src = cfg_path.read_text()
parts = re.split(r'(^\[\[runners\]\])', src, flags=re.MULTILINE)
out = [parts[0]]
i = 1
patched = False
while i < len(parts):
    header = parts[i]
    body = parts[i+1] if i + 1 < len(parts) else ''
    if f'name = "{runner_name}"' in body:
        if 'shell = "sh"' in body:
            patched = True  # already done
        else:
            # If a `shell = "..."` line exists, replace it; else inject one
            new_body, n = re.subn(
                r'^\s*shell = "[^"]*"\n',
                '  shell = "sh"\n',
                body,
                count=1,
                flags=re.MULTILINE,
            )
            if n == 0:
                # Insert after `executor = "shell"` line
                new_body = re.sub(
                    r'(^  executor = "shell"\n)',
                    r'\1  shell = "sh"\n',
                    body,
                    count=1,
                    flags=re.MULTILINE,
                )
            body = new_body
            patched = True
    out.append(header)
    out.append(body)
    i += 2
if not patched:
    print(f"WARN: no [[runners]] matched name='{runner_name}'", file=sys.stderr)
    sys.exit(1)
cfg_path.write_text(''.join(out))
print("patched", file=sys.stderr)
PY

log "Restarting gitlab-runner.service ..."
systemctl restart gitlab-runner.service
sleep 2
systemctl is-active gitlab-runner.service >/dev/null && log "Service active." || {
    log "ERROR: service failed; check journalctl -u gitlab-runner"; exit 1
}

cat <<EOF

✓ Done.

config.toml now sets shell = "sh" for the ${RUNNER_NAME} runner.
POSIX sh skips .bashrc / .bash_logout / .profile sourcing, so the
prepare_environment stage no longer trips on user rc files.

The actual job before_script blocks in .gitlab-ci.yml use 'set -euo
pipefail' and bash-style heredocs which run under bash anyway via the
multiline-string parsing — independent of the wrapper shell.

Trigger any push to main and watch the bench-native jobs succeed.
EOF
