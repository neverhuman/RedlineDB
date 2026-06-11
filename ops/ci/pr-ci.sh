#!/usr/bin/env bash
# RedlineDB hub PR-CI gate — the single authoritative local + CI validate command.
# `bash ops/ci/pr-ci.sh` runs exactly what .github/workflows/ci.yml runs (ci-local
# parity). Green here means the hub is green; it never depends on a sibling repo.
set -Eeuo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"
cd "$(repo_root)"

log_step "installer: shellcheck + syntax"
if command -v shellcheck >/dev/null 2>&1; then shellcheck install.sh ops/ci/*.sh scripts/*.sh; fi
bash -n install.sh

log_step "pointers: family.json valid + README/family agree"
python3 -c "import json; json.load(open('family.json'))"
for r in redline-core redline-testing redline-web; do
  grep -q "neverhuman/$r" README.md   || die "$ERR_POINTER_SYNC" "README missing pointer to $r"
  grep -q "neverhuman/$r" family.json  || die "$ERR_POINTER_SYNC" "family.json missing $r"
  grep -q "neverhuman/$r" FAMILY.md    || die "$ERR_POINTER_SYNC" "FAMILY.md missing $r"
done

log_step "thin-hub invariant: no engine source leaked back in"
# crates/domain/ and crates/hub/ may contain typed exception surfaces (.rs stubs only).
# All other .rs files are engine leaks.
if find . -name '*.rs' -not -path './target/*' -not -path './crates/domain/*' -not -path './crates/hub/*' 2>/dev/null | grep -q .; then
  die "$ERR_ENGINE_LEAKED" ".rs files outside crates/domain/ or crates/hub/ — engine source belongs in redline-core"
fi
if [ -f Cargo.toml ]; then
  die "$ERR_ENGINE_LEAKED" "Cargo.toml found — engine belongs in redline-core, not the hub"
fi

log_step "hub crate tests: typed exception surface"
if command -v cargo >/dev/null 2>&1; then
  cargo test --manifest-path crates/hub/Cargo.toml
else
  log_ok "cargo not available locally — skipping hub crate tests (run in CI)"
fi

log_step "contract drift: re-derive URL template from install.sh"
bash ops/ci/contract-drift.sh

log_step "security lane"
bash ops/ci/security.sh

log_step "jankurai advisory audit"
bash ops/ci/jankurai.sh

log_ok "hub PR-CI: all lanes green"
