#!/usr/bin/env bash
set -euo pipefail
ci_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$ci_root/required.sh"
bash "$ci_root/security.sh"
bash "$ci_root/score.sh"
bash "$ci_root/release-readiness.sh"
