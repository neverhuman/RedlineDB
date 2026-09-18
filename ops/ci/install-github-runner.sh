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

sudo ./svc.sh install "${USER}"
sudo ./svc.sh start
sudo ./svc.sh status
