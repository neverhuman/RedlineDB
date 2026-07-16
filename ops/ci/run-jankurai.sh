#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
source "${repo_root}/ops/ci/jankurai-identity.sh"
run_governed_jankurai "$@"
