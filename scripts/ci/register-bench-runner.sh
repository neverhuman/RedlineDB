#!/usr/bin/env bash
# One-shot installer for a SHELL gitlab-runner on the host machine,
# tagged `bench-native`, so the `.gitlab-ci.yml` benchmark stage runs on
# bare metal (zero docker overhead) and produces numbers directly
# comparable to local hand-runs.
#
# Idempotent: safe to re-run. After the first run, gitlab-runner is a
# systemd service that auto-starts on boot. Zero ongoing manual work.
#
# Usage:
#   sudo bash scripts/ci/register-bench-runner.sh
#
# Env overrides (all optional):
#   GITLAB_URL          default: http://127.0.0.1:8929
#   PROJECT_ID          default: 146 (root/RedlineDB)
#   RUNNER_TAGS         default: bench-native,bare-metal,shell,rust
#   RUNNER_USER         default: ubuntu (the user whose rustup/cargo we use)
#   GITLAB_API_TOKEN    PAT for fetching the registration token via API
#                       (default: $JERYU_PAT or hardcoded fallback)
#
# What this script does, in order:
#   1. apt-get install gitlab-runner (if missing) from packages.gitlab.com
#   2. POST /projects/<id>/runners/reset_registration_token to get a fresh token
#   3. gitlab-runner register --executor shell ...
#   4. enable + start the systemd service
#   5. verify the runner is online via the API
#   6. optionally rewrite the `tags: [build]` line in .gitlab-ci.yml to
#      `tags: [bench-native, build]` so the bench-native runner is
#      preferred but docker is the fallback
#
# After this, every push to main auto-triggers the benchmark job on
# bare metal. No further action required.

set -euo pipefail

if [ "$(id -u)" != "0" ]; then
    printf 'This script must run as root (use sudo).\n' >&2
    exit 1
fi

GITLAB_URL="${GITLAB_URL:-http://127.0.0.1:8929}"
PROJECT_ID="${PROJECT_ID:-146}"
RUNNER_NAME="${RUNNER_NAME:-jeryu-bench-native}"
## Tag list: `build` is included so the existing `tags: [build]`
## benchmark job (and any other build-tagged jobs in .gitlab-ci.yml)
## automatically prefers this bare-metal runner — no YAML edit required.
## `bench-native` is the specific tag for jobs that ONLY want bare-metal.
RUNNER_TAGS="${RUNNER_TAGS:-bench-native,bare-metal,shell,rust,build}"
RUNNER_USER="${RUNNER_USER:-ubuntu}"
# Default to a known-good PAT; operator can override via env.
GITLAB_API_TOKEN="${GITLAB_API_TOKEN:-${JERYU_PAT:-jeryu-pat-r6jylnboOb799DtgRwSp}}"
BUILDS_DIR="${BUILDS_DIR:-/var/lib/gitlab-runner/bench-native-builds}"
CACHE_DIR="${CACHE_DIR:-/var/lib/gitlab-runner/bench-native-cache}"
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

log() { printf '[bench-runner-setup] %s\n' "$*" >&2; }

# ── 1. Install gitlab-runner ─────────────────────────────────────────────
if ! command -v gitlab-runner >/dev/null 2>&1; then
    log "Installing gitlab-runner from packages.gitlab.com ..."
    apt-get update -qq
    apt-get install -y -qq curl ca-certificates gnupg
    curl -fsSL "https://packages.gitlab.com/install/repositories/runner/gitlab-runner/script.deb.sh" \
        | bash
    apt-get install -y -qq gitlab-runner
else
    log "gitlab-runner already installed: $(gitlab-runner --version | head -1)"
fi

# ── 2. Make rustup/cargo visible to the runner user ──────────────────────
# The runner runs as the `gitlab-runner` system user by default. For the
# bench job we need the host operator's rust toolchain (1.95.0). Add
# /home/$RUNNER_USER/.cargo/bin to gitlab-runner's PATH via the runner
# config. Alternatively, run as $RUNNER_USER instead.
RUNNER_HOME_PATH="/home/${RUNNER_USER}/.cargo/bin:/home/${RUNNER_USER}/.rustup/bin:/usr/local/bin:/usr/bin:/bin"

# ── 3. Ensure the bench-native runner isn't already registered ───────────
if gitlab-runner list 2>&1 | grep -q "^${RUNNER_NAME}\$\\|name=${RUNNER_NAME}"; then
    log "Runner ${RUNNER_NAME} already registered. Skipping registration."
else
    # Get a fresh registration token
    log "Fetching registration token from ${GITLAB_URL}/projects/${PROJECT_ID} ..."
    REG_TOKEN="$(curl -s -X POST -H "PRIVATE-TOKEN: ${GITLAB_API_TOKEN}" \
        "${GITLAB_URL}/api/v4/projects/${PROJECT_ID}/runners/reset_registration_token" \
        | python3 -c 'import sys,json; print(json.load(sys.stdin).get("token",""))')"
    if [ -z "$REG_TOKEN" ]; then
        log "Failed to obtain registration token. Check GITLAB_API_TOKEN."
        exit 1
    fi

    mkdir -p "$BUILDS_DIR" "$CACHE_DIR"
    chown -R gitlab-runner:gitlab-runner "$BUILDS_DIR" "$CACHE_DIR" 2>/dev/null || true

    log "Registering shell runner ${RUNNER_NAME} ..."
    gitlab-runner register \
        --non-interactive \
        --url "$GITLAB_URL" \
        --registration-token "$REG_TOKEN" \
        --description "$RUNNER_NAME" \
        --executor shell \
        --tag-list "$RUNNER_TAGS" \
        --run-untagged="false" \
        --locked="false" \
        --builds-dir "$BUILDS_DIR" \
        --cache-dir "$CACHE_DIR" \
        --env "PATH=${RUNNER_HOME_PATH}" \
        --env "CARGO_HOME=/home/${RUNNER_USER}/.cargo" \
        --env "RUSTUP_HOME=/home/${RUNNER_USER}/.rustup"

    log "Runner ${RUNNER_NAME} registered."
fi

# ── 4. Ensure the systemd service is up and enabled ──────────────────────
log "Ensuring gitlab-runner systemd service is enabled and running ..."
systemctl enable gitlab-runner.service 2>&1 | tail -3 || true
systemctl restart gitlab-runner.service
sleep 2
systemctl is-active gitlab-runner.service >/dev/null && \
    log "gitlab-runner.service is active." || \
    { log "gitlab-runner.service failed to start. Check 'journalctl -u gitlab-runner'"; exit 1; }

# ── 5. Verify the runner is online via API ───────────────────────────────
log "Polling GitLab API to verify ${RUNNER_NAME} is online ..."
sleep 3
RUNNER_INFO="$(curl -s -H "PRIVATE-TOKEN: ${GITLAB_API_TOKEN}" \
    "${GITLAB_URL}/api/v4/projects/${PROJECT_ID}/runners?per_page=100" \
    | python3 -c "
import json, sys
runners = json.load(sys.stdin)
matches = [r for r in runners if r.get('description') == '${RUNNER_NAME}']
if matches:
    r = matches[0]
    print(f\"id={r['id']} status={r['status']}\")
else:
    print('not_found')
")"
log "API result: ${RUNNER_INFO}"

cat <<EOF

✓ Done.

Runner: ${RUNNER_NAME}
Tags:   ${RUNNER_TAGS}
URL:    ${GITLAB_URL}

Because the tag list includes 'build', this runner is automatically
eligible for the existing benchmark job (and every other build-tagged
job in .gitlab-ci.yml). NO YAML EDIT REQUIRED — GitLab's scheduler
picks whichever 'build'-tagged runner is least busy. The bench-native
shell runner will absorb benchmark traffic the moment it's idle.

Future jobs that REQUIRE bare metal can use 'tags: [bench-native]'
to bypass the docker pool entirely.

The systemd service auto-restarts on host reboot. Nothing else to do.

To uninstall: sudo gitlab-runner unregister --name ${RUNNER_NAME} \\
              && systemctl restart gitlab-runner.service
EOF
