#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$root"
all=false
case "${1:-}" in
  --all) all=true; shift ;;
  --help|-h) printf 'Usage: %s [--all]\n' "$0"; exit 0 ;;
esac
[[ $# == 0 ]] || { printf 'Usage: %s [--all]\n' "$0" >&2; exit 64; }
for tool in cargo rustc cc pkg-config; do
  command -v "$tool" >/dev/null || { printf 'missing prerequisite: %s\n' "$tool" >&2; exit 1; }
done
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$root/target}
mkdir -p "$CARGO_TARGET_DIR"
CARGO_TARGET_DIR=$(cd "$CARGO_TARGET_DIR" && pwd -P)
cargo build --locked --release -p redlinedb-cli -p redlinedb-server -p redlinedb-ffi
if "$all"; then
  command -v npm >/dev/null || { printf 'missing prerequisite for --all: npm\n' >&2; exit 1; }
  npm --prefix subrepos/redline-web/apps/web ci
  npm --prefix subrepos/redline-web/apps/web run build
  for component in redline-testing redline-central redline-web redline-split-ops; do
    cargo build --locked --release --manifest-path "subrepos/$component/Cargo.toml" --workspace --bins
  done
fi
printf 'Build complete. Install with ./scripts/install-from-source.sh (add --all for supporting tools).\n'
