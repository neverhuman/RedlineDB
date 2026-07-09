#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

./ops/ci/required.sh
./ops/ci/security.sh

printf 'check ok: jain-split-ops\n'
