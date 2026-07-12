#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$repo_root/ops/ci/lib.sh"
for tool in git cargo rustc flock cargo-audit cargo-deny gitleaks actionlint zizmor syft; do
  require_tool "$tool"
done
require_jankurai
"$repo_root/redlinectl" doctor
