#!/usr/bin/env bash
#
# One-command setup for redline-testing. Installs the pinned Rust toolchain,
# fetches dependencies, and builds the workspace so that `bash ops/ci/pr-ci.sh`
# (the single validate command) is ready to run.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

say() { printf '[setup] %s\n' "$*"; }

if ! command -v cargo >/dev/null 2>&1; then
    say "cargo not found — install Rust via https://rustup.rs and re-run"
    exit 1
fi

# rust-toolchain.toml pins the channel; this materializes it.
say "resolving pinned toolchain"
rustup show active-toolchain >/dev/null 2>&1 || true

say "fetching dependencies"
cargo fetch --locked

say "building workspace"
cargo build --workspace --locked

cat <<'NEXT'
[setup] done.

Validate with the single command:
    bash ops/ci/pr-ci.sh

Other lanes:
    bash ops/ci/security.sh    # secret + dependency + workflow scans
    bash ops/ci/jankurai.sh    # jankurai tool-suite evidence
NEXT
