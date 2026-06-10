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

# Security + supply-chain lane only.
security:
    bash ops/ci/security.sh

# Jankurai advisory audit.
score:
    mkdir -p target/jankurai
    "${JANKURAI_BIN:-$HOME/.cargo/bin/jankurai}" audit . --mode advisory \
      --policy agent/audit-policy.toml \
      --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md
