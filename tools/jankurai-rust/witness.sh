#!/usr/bin/env bash
set -euo pipefail
repo_root="${1:-.}"
exec bash ops/ci/run-jankurai.sh rust witness build "$repo_root"
