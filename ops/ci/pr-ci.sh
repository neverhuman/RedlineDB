#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"
bash scripts/guard-no-duplicate-engine.sh
bash ops/ci/no-python-policy.sh
just test
bash ops/ci/evidence-processor.sh
bash scripts/ci-doctor.sh
