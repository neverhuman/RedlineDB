#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

# Sealed host-CI sandboxes do not ship `rtk` on PATH. Match the redline-testing
# dispatcher: provide a passthrough shim so ci-doctor and just recipes stay
# runnable without requiring a host-local rtk install inside the sandbox.
if ! command -v rtk >/dev/null 2>&1; then
  rtk() {
    "$@"
  }
  export -f rtk
fi

bash scripts/guard-no-duplicate-engine.sh
bash ops/ci/no-python-policy.sh
bash ops/ci/evidence-processor.sh
bash scripts/ci-doctor.sh
