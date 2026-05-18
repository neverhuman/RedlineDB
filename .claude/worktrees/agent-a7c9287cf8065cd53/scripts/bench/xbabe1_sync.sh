#!/usr/bin/env bash
set -euo pipefail

REMOTE="${REMOTE:-xbabe1}"
REMOTE_DIR="${REMOTE_DIR:-/home/ubuntu/RedlineDB}"

rsync -a --delete \
  --exclude '.git/' \
  --exclude 'target/' \
  ./ "${REMOTE}:${REMOTE_DIR}/"
