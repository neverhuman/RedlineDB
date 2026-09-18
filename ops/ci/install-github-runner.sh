#!/usr/bin/env bash
# Install one GitHub Actions self-hosted runner instance.
#
# Required env:
#   GITHUB_RUNNER_TOKEN  registration token from
#     `gh api -X POST repos/neverhuman/RedlineDB/actions/runners/registration-token`
#   RUNNER_NAME          unique name, e.g. xbabe2-1
#   RUNNER_LABELS        comma-separated, e.g. self-hosted,linux,x64,xbabe2
#
# Optional:
#   RUNNER_DIR           install directory (default ~/actions-runners/$RUNNER_NAME)
#   RUNNER_VERSION       default 2.337.0
#   RUNNER_REPO_URL      default https://github.com/neverhuman/RedlineDB
set -euo pipefail

if [[ -z "${GITHUB_RUNNER_TOKEN:-}" ]]; then
  echo "GITHUB_RUNNER_TOKEN is required" >&2
  exit 64
fi
if [[ -z "${RUNNER_NAME:-}" ]]; then
  echo "RUNNER_NAME is required" >&2
  exit 64
fi
if [[ -z "${RUNNER_LABELS:-}" ]]; then
  echo "RUNNER_LABELS is required" >&2
  exit 64
fi

VERSION="${RUNNER_VERSION:-2.337.0}"
SHA256="${RUNNER_SHA256:-70920811a4f8ad4328818682bca5c6469c1c942fab52448868071d0063816613}"
TARBALL="actions-runner-linux-x64-${VERSION}.tar.gz"
URL="https://github.com/actions/runner/releases/download/v${VERSION}/${TARBALL}"
REPO_URL="${RUNNER_REPO_URL:-https://github.com/neverhuman/RedlineDB}"
DIR="${RUNNER_DIR:-${HOME}/actions-runners/${RUNNER_NAME}}"

mkdir -p "${DIR}"
cd "${DIR}"

if [[ ! -f "${TARBALL}" ]]; then
  curl -fsSL -o "${TARBALL}" "${URL}"
fi
echo "${SHA256}  ${TARBALL}" | sha256sum -c -

if [[ ! -x ./config.sh ]]; then
  tar xzf "${TARBALL}"
fi

if [[ -f .runner ]]; then
  echo "runner already configured in ${DIR}"
else
  ./config.sh --unattended \
    --url "${REPO_URL}" \
    --token "${GITHUB_RUNNER_TOKEN}" \
    --name "${RUNNER_NAME}" \
    --labels "${RUNNER_LABELS}" \
    --work _work \
    --replace
fi

HOOK_DIR="$(cd "${DIR}/.." && pwd)"
HOOK="${HOOK_DIR}/job-started.sh"
cat > "${HOOK}" <<'EOS'
#!/usr/bin/env bash
# GitHub Actions job_started hook. Self-hosted workdirs persist; tests
# may chmod 0444 directories that actions/checkout cannot unlink.
set +e
if [ -n "${GITHUB_WORKSPACE}" ] && [ -d "${GITHUB_WORKSPACE}" ]; then
  chmod -R u+rwx "${GITHUB_WORKSPACE}"
fi
if [ -n "${RUNNER_TOOL_CACHE}" ] && [ -d "${RUNNER_TOOL_CACHE}/redlinedb-target" ]; then
  chmod -R u+rwx "${RUNNER_TOOL_CACHE}/redlinedb-target"
fi
exit 0
EOS
chmod 0755 "${HOOK}"
if [[ -f .env ]]; then
  if ! grep -q '^ACTIONS_RUNNER_HOOK_JOB_STARTED=' .env; then
    printf '\nACTIONS_RUNNER_HOOK_JOB_STARTED=%s\n' "${HOOK}" >> .env
  fi
else
  printf 'ACTIONS_RUNNER_HOOK_JOB_STARTED=%s\n' "${HOOK}" > .env
fi

sudo mkdir -p "/etc/systemd/system/actions.runner.neverhuman-RedlineDB.${RUNNER_NAME}.service.d"
sudo tee "/etc/systemd/system/actions.runner.neverhuman-RedlineDB.${RUNNER_NAME}.service.d/path.conf" >/dev/null <<'EOS'
[Service]
Environment=PATH=/home/ubuntu/.cargo/bin:/home/ubuntu/.local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
EOS

sudo ./svc.sh install "${USER}"
sudo systemctl daemon-reload
sudo ./svc.sh start
sudo ./svc.sh status
