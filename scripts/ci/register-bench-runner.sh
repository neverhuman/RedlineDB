#!/usr/bin/env bash
# Register a SHELL gitlab-runner directly on the host (no docker), tagged
# `bench-native`, so the benchmark stage runs against bare-metal CPU
# performance with zero container overhead.
#
# Why bare-metal: the docker-executor runners (`build`, `default` pools)
# add ~2-5% syscall overhead on syscall-heavy workloads like the
# sqlite_parity corpus. For perf-bench numbers we want to publish, the
# JSONL artifact should reflect real-host performance, not
# container-tax-discounted performance.
#
# When this runner is registered, edit `.gitlab-ci.yml`'s `benchmark`
# job and change `tags: [build]` to `tags: [bench-native]` so the
# benchmark stage prefers the bare-metal runner.
#
# Prereqs (run as root or with sudo):
#   1. Install gitlab-runner:
#        curl -L "https://packages.gitlab.com/runner/gitlab-runner/gpgkey" | apt-key add -
#        curl -L "https://packages.gitlab.com/install/repositories/runner/gitlab-runner/script.deb.sh" | bash
#        apt-get install -y gitlab-runner
#   2. Get a project-scoped registration token from
#        http://gitlab.local:8929/root/RedlineDB/-/settings/ci_cd#js-runners-settings
#      OR via API:
#        curl -s -H "PRIVATE-TOKEN: $JERYU_PAT" \
#          "http://127.0.0.1:8929/api/v4/projects/146/runners/reset_registration_token" \
#          -X POST | jq -r .token
#   3. Install build deps:
#        apt-get install -y build-essential clang mold sqlite3 zlib1g-dev curl ca-certificates git jq python3
#   4. Install rust toolchain (project pin is 1.95.0):
#        rustup toolchain install 1.95.0 --default
#
# Usage:
#   sudo REGISTRATION_TOKEN=<token> bash scripts/ci/register-bench-runner.sh
#
# After this script completes, the runner is registered with GitLab and
# starts polling for jobs tagged `bench-native`.
#
# To uninstall: `sudo gitlab-runner unregister --name jeryu-bench-native`

set -euo pipefail

: "${REGISTRATION_TOKEN:?Set REGISTRATION_TOKEN to a project-scoped GitLab runner token}"

GITLAB_URL="${GITLAB_URL:-http://127.0.0.1:8929}"
RUNNER_NAME="${RUNNER_NAME:-jeryu-bench-native}"
RUNNER_TAGS="${RUNNER_TAGS:-bench-native,bare-metal,shell,rust}"
RUNNER_CONCURRENT="${RUNNER_CONCURRENT:-1}"
BUILDS_DIR="${BUILDS_DIR:-/var/lib/gitlab-runner/bench-native-builds}"
CACHE_DIR="${CACHE_DIR:-/var/lib/gitlab-runner/bench-native-cache}"

if ! command -v gitlab-runner >/dev/null 2>&1; then
    printf 'gitlab-runner CLI not found — install from %s\n' \
        'https://docs.gitlab.com/runner/install/' >&2
    exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
    printf 'cargo not found — install rustup + toolchain 1.95.0 first\n' >&2
    exit 1
fi

mkdir -p "$BUILDS_DIR" "$CACHE_DIR"

printf 'Registering shell runner "%s" with %s ...\n' \
    "$RUNNER_NAME" "$GITLAB_URL" >&2

gitlab-runner register \
    --non-interactive \
    --url "$GITLAB_URL" \
    --registration-token "$REGISTRATION_TOKEN" \
    --description "$RUNNER_NAME" \
    --executor shell \
    --tag-list "$RUNNER_TAGS" \
    --run-untagged="false" \
    --locked="false" \
    --builds-dir "$BUILDS_DIR" \
    --cache-dir "$CACHE_DIR"

printf '\nShell runner "%s" registered.\n' "$RUNNER_NAME" >&2
printf 'Next step: edit .gitlab-ci.yml `benchmark:` and change\n' >&2
printf '  tags: [build]\n' >&2
printf 'to:\n' >&2
printf '  tags: [bench-native]\n' >&2
printf 'so the benchmark stage prefers the bare-metal runner.\n' >&2
