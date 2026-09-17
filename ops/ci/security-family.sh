#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
# shellcheck source=ops/ci/lib.sh
source ops/ci/lib.sh
CI_GITLEAKS_INSTALL_DIR="$root/target/ci/tools" ci_install_gitleaks
snapshot=$(mktemp -d)
trap 'rm -rf "$snapshot"' EXIT
git ls-files -z | tar --null -T - -cf - | tar -xf - -C "$snapshot"
"$root/target/ci/tools/gitleaks" detect --no-git --source "$snapshot" --config "$root/.gitleaks.toml" --redact
for component in . subrepos/redline-testing subrepos/redline-central subrepos/redline-web subrepos/redline-split-ops; do
  (cd "$component"; cargo audit --file Cargo.lock; if [[ -f deny.toml ]]; then cargo deny check; fi)
done
npm --prefix subrepos/redline-web/apps/web ci
npm --prefix subrepos/redline-web/apps/web audit --audit-level=high
