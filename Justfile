# RedlineDB hub — recipes. The hub is a thin front-door (docs, installer, release glue).
set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# List recipes.
default:
    @just --list

# One-command setup (tool check).
setup:
    bash scripts/setup.sh

# The single validate command — identical locally and in CI (ci-local parity).
check:
    bash ops/ci/pr-ci.sh

# Alias.
ci: check

# Fast deterministic iteration: syntax + contract drift + pointer checks (no slow scans).
fast:
    bash -n install.sh
    bash ops/ci/contract-drift.sh
    python3 -c "import json; json.load(open('family.json'))"

# Security + supply-chain lane only.
security:
    bash ops/ci/security.sh

# Fuse the three sibling repos into .fusion/ for end-to-end local dev (GitHub source).
# End users never need this — they just install the binary. Then: ./.fusion/dev.sh build-all
fuse:
    bash scripts/fuse.sh

# Same, but clone/update the siblings from the internal jeryu forge instead of GitHub.
fuse-jeryu:
    FUSION_SOURCE=jeryu bash scripts/fuse.sh

# Run hub crate tests with nextest (fast parallel runner; falls back to cargo test).
test:
    cargo nextest run --manifest-path crates/hub/Cargo.toml 2>/dev/null \
      || cargo test --manifest-path crates/hub/Cargo.toml

# Run hub tests with standard cargo test only (CI fallback).
test-cargo:
    cargo test --manifest-path crates/hub/Cargo.toml

# Jankurai advisory audit.
score:
    mkdir -p target/jankurai
    "${JANKURAI_BIN:-$HOME/.cargo/bin/jankurai}" audit . --mode advisory \
      --policy agent/audit-policy.toml \
      --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md
