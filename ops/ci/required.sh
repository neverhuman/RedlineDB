#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

bash ops/ci/fast.sh
bash ops/ci/security.sh
bash ops/ci/jankurai.sh
bash ops/ci/contract-drift.sh
bash ops/ci/artifact-support-test.sh
bash ops/ci/coverage.sh
