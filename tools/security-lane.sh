#!/usr/bin/env bash
set -euo pipefail

# Visible security command inventory for Jankurai:
# gitleaks detect --source . --no-git --redact --exit-code 1
# jankurai security run . --out target/jankurai/security/evidence.json
# syft dir:. --output spdx-json
# cargo audit

bash ops/ci/security.sh

