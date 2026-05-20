#!/usr/bin/env bash
set -euo pipefail

if [ "${RUNNER_OS:-}" = "Linux" ]; then
  sudo apt-get update
  sudo apt-get install -y mold
fi
