#!/usr/bin/env bash
# RedlineDB hub PR-CI gate — the single authoritative local + CI validate command.
# `bash ops/ci/pr-ci.sh` runs exactly what .github/workflows/ci.yml runs (ci-local
# parity). Green here means the hub is green; it never depends on a sibling repo.
set -Eeuo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

echo "==> installer: shellcheck + syntax"
if command -v shellcheck >/dev/null 2>&1; then shellcheck install.sh ops/ci/*.sh scripts/*.sh; fi
bash -n install.sh

echo "==> pointers: family.json valid + README/family agree"
python3 -c "import json; json.load(open('family.json'))"
for r in redline-core redline-testing redline-web; do
  grep -q "neverhuman/$r" README.md   || { echo "README missing pointer to $r"; exit 1; }
  grep -q "neverhuman/$r" family.json  || { echo "family.json missing $r"; exit 1; }
  grep -q "neverhuman/$r" FAMILY.md    || { echo "FAMILY.md missing $r"; exit 1; }
done

echo "==> thin-hub invariant: no engine source leaked back in"
if [ -d crates ] || [ -f Cargo.toml ]; then echo "engine belongs in redline-core, not the hub"; exit 1; fi

echo "==> security lane"
bash ops/ci/security.sh

echo "==> jankurai advisory audit"
mkdir -p target/jankurai
JANKURAI="${JANKURAI_BIN:-$HOME/.cargo/bin/jankurai}"
[ -x "$JANKURAI" ] || JANKURAI="$(command -v jankurai || true)"
if [ -n "${JANKURAI:-}" ] && [ -x "$JANKURAI" ]; then
  "$JANKURAI" audit . --mode advisory --policy agent/audit-policy.toml \
    --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md || true
fi

echo "==> hub PR-CI: OK"
