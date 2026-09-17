#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
bash scripts/ci-family.sh all
bash ops/ci/install-github-tools.sh
target/ci/tools/jankurai security run . --strict --profile ci --script ops/ci/security-family.sh --out target/jankurai/security/evidence.json
bash ops/ci/audit-family.sh
