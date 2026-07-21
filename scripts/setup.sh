#!/usr/bin/env bash
#
# One-command setup for redline-testing. Uses the already-pinned Rust toolchain
# and in-tree Cargo custody, and builds the workspace so `bash ops/ci/pr-ci.sh`
# (the single validate command) is ready to run.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

say() { printf '[setup] %s\n' "$*"; }

if ! command -v cargo >/dev/null 2>&1; then
    say "cargo not found; provision the pinned toolchain into local custody"
    exit 1
fi

custody_home="${REDLINE_CARGO_HOME:-$repo_root/../../target/redline-testing-cargo-home}"
[[ -d "$custody_home" && ! -L "$custody_home" ]] || {
    say "physical in-tree Cargo custody is missing: $custody_home"
    exit 1
}
export CARGO_HOME="$custody_home"
export CARGO_NET_OFFLINE=true

say "building workspace from offline custody"
cargo build --workspace --locked --offline

cat <<'NEXT'
[setup] done.

Validate with the single command:
    bash ops/ci/pr-ci.sh

Other lanes:
    bash ops/ci/security.sh    # secret + dependency + workflow scans
    bash ops/ci/jankurai.sh    # jankurai tool-suite evidence
NEXT
