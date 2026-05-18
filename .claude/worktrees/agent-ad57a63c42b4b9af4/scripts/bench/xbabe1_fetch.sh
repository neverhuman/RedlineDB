#!/usr/bin/env bash
set -euo pipefail

# Lane BH P1 #4: artifact path layout.
#
# The certify writer puts results under
#   <REMOTE_DIR>/target/bench/xbabe1/<name>/...
# so this fetch script preserves the `xbabe1` prefix locally:
#   <LOCAL_ROOT>/<name>/...
# (LOCAL_ROOT defaults to `target/bench/xbabe1`, keeping the remote
#  prefix so xbabe1 results never overwrite locally-produced ones.)
#
# Pre-Lane-BH this script mapped `--name=certification` to
# `target/bench/certification` instead of `target/bench/xbabe1/certification`,
# silently rsyncing from the wrong remote path.

REMOTE="${REMOTE:-xbabe1}"
REMOTE_DIR="${REMOTE_DIR:-/home/ubuntu/RedlineDB}"
LOCAL_ROOT="${LOCAL_ROOT:-target/bench/xbabe1}"
STAMP="${1:-}"

if [ -z "${STAMP}" ]; then
  LATEST_REMOTE="$(ssh "${REMOTE}" "ls -1dt '${REMOTE_DIR}'/target/bench/xbabe1/* 2>/dev/null | head -n 1 || true")"
  if [ -z "${LATEST_REMOTE}" ]; then
    echo "no remote benchmark artifacts found under target/bench/xbabe1/" >&2
    exit 1
  fi
  STAMP="$(basename "${LATEST_REMOTE}")"
fi

mkdir -p "${LOCAL_ROOT}/${STAMP}"
rsync -a --delete \
  "${REMOTE}:${REMOTE_DIR}/target/bench/xbabe1/${STAMP}/" \
  "${LOCAL_ROOT}/${STAMP}/"
