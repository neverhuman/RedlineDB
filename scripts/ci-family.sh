#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
export CARGO_TARGET_DIR="$root/target"
case "${1:-all}" in
  engine) bash ops/ci/fast.sh ;;
  testing)
    cargo fmt --manifest-path subrepos/redline-testing/Cargo.toml --check
    cargo test --locked --manifest-path subrepos/redline-testing/Cargo.toml --workspace
    ;;
  central)
    cargo test --locked --manifest-path subrepos/redline-central/Cargo.toml --workspace --all-targets
    bash subrepos/redline-central/ops/ci/backend-dependency-guard.sh
    ;;
  web)
    npm --prefix subrepos/redline-web/apps/web ci
    npm --prefix subrepos/redline-web/apps/web run ci
    cargo test --locked --manifest-path subrepos/redline-web/Cargo.toml --workspace --all-targets
    ;;
  release-tools)
    cargo fmt --manifest-path subrepos/redline-split-ops/Cargo.toml --check
    cargo test --locked --manifest-path subrepos/redline-split-ops/Cargo.toml
    cargo run --locked --manifest-path subrepos/redline-split-ops/Cargo.toml -- validate
    ;;
  integration)
    ./scripts/build-from-source.sh --all
    bash scripts/test-binaries.sh "$root/target/release"
    (cd subrepos/redline-web/apps/web; npx --no-install playwright install chromium; REDLINE_WEB_TARGET_BIN="$root/target/release/redlinedb" npx --no-install playwright test)
    ;;
  parity) bash ops/ci/parity.sh ;;
  packaging) TAG=${TAG:-v4.1.0-rc.1} bash scripts/package-release.sh; bash scripts/test-installer.sh ;;
  all)
    for lane in engine testing central web release-tools integration parity packaging; do "$0" "$lane"; done
    ;;
  *) printf 'Unknown family CI lane: %s\n' "$1" >&2; exit 64 ;;
esac
