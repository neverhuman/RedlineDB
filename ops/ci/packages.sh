#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
case "${1:?package lane required}" in
  build) exec bash scripts/package-release.sh ;;
  smoke) exec bash scripts/test-binaries.sh ;;
  installer) exec bash scripts/test-installer.sh ;;
  runtime) exec bash scripts/test-packages.sh ;;
  *) exit 64 ;;
esac
