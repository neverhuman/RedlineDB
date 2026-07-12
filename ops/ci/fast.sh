#!/usr/bin/env bash
# Deterministic fast lane: shell syntax, Rust fmt/check, npm lockfile, actions.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"

log "fast: checking shell syntax"
while IFS= read -r script; do
  bash -n "$script"
done < <(find scripts ops/ci ops/git-hooks -type f -name '*.sh' 2>/dev/null | sort)

log "fast: checking CI language boundary"
cargo fmt --manifest-path tools/release-control/Cargo.toml -- --check
cargo clippy --locked --manifest-path tools/release-control/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path tools/release-control/Cargo.toml
cargo run --quiet --locked --manifest-path tools/release-control/Cargo.toml -- \
  language-boundary .

if repo_has Cargo.toml && ! has cargo; then
  missing_tool cargo "Rust formatting and checks"
elif cargo_workspace_ready; then
  log "fast: rust fmt + check"
  cargo fmt --all -- --check
  cargo check --workspace --all-targets --locked
elif repo_has Cargo.toml; then
  warn "skipping Rust checks: Cargo workspace metadata not ready"
fi

if repo_has apps/web/package-lock.json && has npm; then
  log "fast: verifying npm lockfile"
  (cd "$WEB_DIR" && npm ci --ignore-scripts --no-audit --no-fund)
elif repo_has apps/web/package-lock.json; then
  missing_tool npm "npm lockfile verification"
fi

if has actionlint; then
  log "fast: linting GitHub Actions"
  (cd "$ROOT_DIR" && actionlint)
else
  missing_tool actionlint "GitHub Actions linting"
fi

log "fast: complete"
