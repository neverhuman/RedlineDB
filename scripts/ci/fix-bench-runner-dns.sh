#!/usr/bin/env bash
# Patch the running bench-native shell runner so it can resolve the
# `gitlab.local` hostname that GitLab uses in CI_REPOSITORY_URL.
#
# The docker-executor runners resolve `gitlab.local` via the docker
# network's extra_hosts mapping (see ~/.jeryu/runners/*/config.toml).
# The shell runner runs on the bare host and inherits the host's
# /etc/hosts, which doesn't include `gitlab.local` by default.
#
# This script:
#   1. Adds `127.0.0.1  gitlab.local` to /etc/hosts (idempotent)
#   2. Adds a `pre_get_sources_script` to the gitlab-runner config that
#      rewrites gitlab.local URLs to 127.0.0.1 via git insteadOf (belt
#      and suspenders — works even if /etc/hosts gets rotated out)
#   3. Restarts gitlab-runner.service
#
# Usage:
#   sudo bash scripts/ci/fix-bench-runner-dns.sh
#
# Re-runnable. Idempotent.

set -euo pipefail

if [ "$(id -u)" != "0" ]; then
    printf 'Run as root (sudo).\n' >&2
    exit 1
fi

GITLAB_RUNNER_CONFIG="${GITLAB_RUNNER_CONFIG:-/etc/gitlab-runner/config.toml}"
HOSTS_FILE="${HOSTS_FILE:-/etc/hosts}"

log() { printf '[fix-bench-dns] %s\n' "$*" >&2; }

# ── 1. /etc/hosts ────────────────────────────────────────────────────────
if grep -qE '^[^#]*\bgitlab\.local\b' "$HOSTS_FILE" 2>/dev/null; then
    log "/etc/hosts already maps gitlab.local — leaving as-is."
else
    log "Adding '127.0.0.1  gitlab.local' to ${HOSTS_FILE} ..."
    printf '\n# Added by RedlineDB bench-native runner setup\n127.0.0.1\tgitlab.local\n' \
        >> "$HOSTS_FILE"
fi

# ── 2. git insteadOf via pre_get_sources_script ──────────────────────────
# Find the bench-native runner section in the config and add (or update)
# pre_get_sources_script. Use a Python helper because TOML is finicky in
# bash sed.
if [ ! -f "$GITLAB_RUNNER_CONFIG" ]; then
    log "ERROR: ${GITLAB_RUNNER_CONFIG} not found. Is gitlab-runner installed?"
    exit 1
fi

PATCH_SCRIPT='git config --global url.http://127.0.0.1:8929/.insteadOf http://gitlab.local:8929/'

# Quick check: does the bench-native section already include the patch?
if grep -qF "$PATCH_SCRIPT" "$GITLAB_RUNNER_CONFIG"; then
    log "git insteadOf rewrite already in runner config."
else
    log "Patching ${GITLAB_RUNNER_CONFIG} to add git insteadOf rewrite ..."
    # Use Python to safely insert a pre_get_sources_script line into the
    # bench-native [[runners]] block.
    python3 - "$GITLAB_RUNNER_CONFIG" "$PATCH_SCRIPT" <<'PY'
import sys, pathlib
path = pathlib.Path(sys.argv[1])
patch = sys.argv[2]
text = path.read_text()
# Find every [[runners]] block, look for one whose name = "jeryu-bench-native".
lines = text.splitlines()
out = []
i = 0
in_target = False
patched = False
while i < len(lines):
    line = lines[i]
    if line.strip().startswith("[[runners]]"):
        in_target = False
    if in_target and not patched and line.strip().startswith("executor "):
        # Insert pre_get_sources_script just after executor=...
        out.append(line)
        out.append(f'  pre_get_sources_script = "{patch}"')
        patched = True
        i += 1
        continue
    if "jeryu-bench-native" in line:
        in_target = True
    out.append(line)
    i += 1
if not patched:
    print("WARN: did not find jeryu-bench-native [[runners]] block to patch", file=sys.stderr)
    sys.exit(0)
path.write_text("\n".join(out) + "\n")
PY
fi

# ── 3. Restart the service ──────────────────────────────────────────────
log "Restarting gitlab-runner.service ..."
systemctl restart gitlab-runner.service
sleep 2
systemctl is-active gitlab-runner.service >/dev/null && log "Service active." || {
    log "ERROR: service failed to start. Check journalctl -u gitlab-runner."
    exit 1
}

# ── 4. Smoke test ───────────────────────────────────────────────────────
log "Smoke test: resolving gitlab.local from this shell ..."
if getent hosts gitlab.local >/dev/null; then
    log "  gitlab.local resolves: $(getent hosts gitlab.local | awk '{print $1}')"
else
    log "  WARN: gitlab.local does not resolve via getent (NSS may be cached)."
fi

cat <<EOF

✓ Done.

What changed:
  - /etc/hosts now maps gitlab.local → 127.0.0.1
  - ${GITLAB_RUNNER_CONFIG} has a pre_get_sources_script rewriting
    gitlab.local → 127.0.0.1 via 'git config --global url.insteadOf'
    (belt and suspenders)
  - gitlab-runner.service restarted

Trigger a fresh pipeline (push an empty commit on main, or click "Run
pipeline" in the UI) to confirm the bench-native runner now successfully
fetches sources.
EOF
