# redline-web — dev + CI recipes
set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# List recipes.
default:
    @just --list

# ---- one-command setup + validate ------------------------------------------

# Install deps, build the frontend, build the binary (embeds the frontend).
setup:
    bash scripts/setup.sh

# The single validate command: same lanes locally and in CI (ci-local parity).
check:
    bash ops/ci/pr-ci.sh

# Alias for `check`.
verify: check

# Full PR-CI gate locally (identical to the jeryu / GitHub Actions check).
ci:
    bash ops/ci/pr-ci.sh

# Fast deterministic lane (shell, fmt, check, lockfile, actionlint).
fast:
    bash ops/ci/fast.sh

# Doctor: confirm the local toolchain matches CI.
doctor:
    bash scripts/ci-doctor.sh

# ---- build / dev ------------------------------------------------------------

# Build the frontend, then the embedded release binary.
build:
    cd apps/web && (npm ci --no-audit --no-fund || npm install --no-audit --no-fund) && npm run build
    cargo build --release --locked

# Backend dev server against a SQLite file (seeds a demo db if empty).
dev-server db="/tmp/redline-web-dev.sqlite":
    cargo run -p redline-web-server -- --db {{db}}

# Frontend dev server (Vite, proxies /api + /metrics to :7788).
dev-web:
    cd apps/web && npm run dev

# Auto-format Rust + frontend.
fmt:
    cargo fmt --all
    cd apps/web && node ./node_modules/eslint/bin/eslint.js . --fix || true

# ---- lanes ------------------------------------------------------------------

web:
    bash ops/ci/web.sh

backend:
    bash ops/ci/backend.sh

security:
    bash ops/ci/security.sh

e2e:
    bash ops/ci/e2e.sh

ux-qa:
    bash ops/ci/ux-qa.sh

contract-drift:
    bash ops/ci/contract-drift.sh

cost-budget:
    bash ops/ci/cost-budget.sh

release-readiness:
    bash ops/ci/release-readiness.sh

# ---- jankurai tool suite (evidence under target/jankurai/**) ----------------

# audit-ci: advisory repo score.
score:
    mkdir -p target/jankurai
    "${JANKURAI_BIN:-$HOME/.cargo/bin/jankurai}" audit . --mode advisory \
      --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md

# Run the full jankurai tool suite.
jankurai:
    bash ops/ci/jankurai.sh

proof-routing:
    "${JANKURAI_BIN:-$HOME/.cargo/bin/jankurai}" proof . --changed-from "${JANKURAI_BASE_REF:-origin/main}" --out target/jankurai/proof-routing.json --md target/jankurai/proof-routing.md

proofbind:
    "${JANKURAI_BIN:-$HOME/.cargo/bin/jankurai}" proofbind verify . --changed-from "${JANKURAI_BASE_REF:-origin/main}" --out target/jankurai/proofbind/surface-witness.json --obligations-out target/jankurai/proofbind/obligations.json

proofmark-rust:
    "${JANKURAI_BIN:-$HOME/.cargo/bin/jankurai}" proofmark rust . --obligations target/jankurai/proofbind/obligations.json --out target/jankurai/proofmark/proofmark-receipt.json

copy-code:
    "${JANKURAI_BIN:-$HOME/.cargo/bin/jankurai}" copy-code . --json target/jankurai/copy-code.json --md target/jankurai/copy-code.md

security-evidence:
    "${JANKURAI_BIN:-$HOME/.cargo/bin/jankurai}" security run . --script ops/ci/security.sh --out target/jankurai/security/evidence.json

language-bad-behavior:
    bash ops/ci/language-bad-behavior.sh

rust-witness:
    "${JANKURAI_BIN:-$HOME/.cargo/bin/jankurai}" rust witness build . --out target/jankurai/rust/witness-graph.json

# authz-matrix / input-boundary / agent-tool-supply are audit detectors; the
# audit produces their evidence in the repo score JSON.
authz-matrix:
    "${JANKURAI_BIN:-$HOME/.cargo/bin/jankurai}" audit . --mode advisory --json target/jankurai/authz-matrix.json --md target/jankurai/authz-matrix.md

input-boundary:
    "${JANKURAI_BIN:-$HOME/.cargo/bin/jankurai}" audit . --mode advisory --json target/jankurai/input-boundary.json --md target/jankurai/input-boundary.md

agent-tool-supply:
    "${JANKURAI_BIN:-$HOME/.cargo/bin/jankurai}" audit . --mode advisory --json target/jankurai/agent-tool-supply.json --md target/jankurai/agent-tool-supply.md

evidence-catalog:
    bash ops/ci/evidence-catalog.sh
