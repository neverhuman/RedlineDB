set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# Default recipe runs the same lane CI runs.
default: pr-ci

# One-command setup: install/build everything needed to validate.
setup:
    bash scripts/setup.sh

# Fast iteration loop: fmt check + compile check + nextest (no release packaging).
fast:
    cargo fmt --check
    cargo check --locked --all-targets
    cargo nextest run --workspace --no-fail-fast

# Fast format + compile check (matches the start of pr-ci, no test run).
check:
    cargo fmt --check
    cargo check --locked --all-targets

# Run the Rust test suite (28 integration + unit tests).
test:
    cargo test --locked

# Fast targeted test loop via nextest (narrow per-package runs + caching).
test-fast:
    cargo nextest run --workspace --no-fail-fast
    cargo test -p redline-testing --doc

# Full verification mirror of the primary CI validation pipeline.
verify: pr-ci

pr-ci:
    bash ops/ci/pr-ci.sh

# Security lane: gitleaks + cargo-audit + cargo-deny + zizmor + SBOM.
security:
    bash ops/ci/security.sh

# Jankurai tool-suite evidence lane (artifacts under target/jankurai/**).
jankurai:
    bash ops/ci/jankurai.sh

# Run the jankurai audit and refresh the local score.
score:
    jankurai audit . --mode advisory --json .jankurai/repo-score.json --md .jankurai/repo-score.md

# Build + package the release tarball + manifest. Refactored to invoke
# scripts/release-package.sh so the artifact_hashes set is glob-driven (every
# file under dist/<package>/ is enumerated, not hand-listed). New corpora /
# schemas / templates auto-appear without editing this recipe.
release-local:
    scripts/release-package.sh

# Run the corpus generator's drift check (re-emits matrix-product shards and
# fails on diff). Wired into pr-ci via xtask alongside the existing checks.
generate-check:
    cargo run -p xtask --release -- generate --check

# Validate every shard under corpus/sqlite_parity/cases/ against sqlite3.
ship-gate:
    cargo run -p xtask --release -- ship-gate

# Entry point for the protected redline-testing/required check: the existing lane, unchanged.
required: pr-ci
