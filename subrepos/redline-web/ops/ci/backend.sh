#!/usr/bin/env bash
# Backend lane: fmt, clippy, tests, release build. The release build embeds the
# already-built apps/web/dist, so run ops/ci/web.sh first.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"

if ! has cargo; then
  missing_tool cargo "Rust backend"
  exit 0
fi

if [[ ! -d "${WEB_DIR}/dist" ]]; then
  warn "backend: apps/web/dist missing; running web lane first"
  bash "${ROOT_DIR}/ops/ci/web.sh"
fi

log "backend: fmt"
cargo fmt --all -- --check
log "backend: clippy"
cargo clippy --workspace --all-targets --locked -- -D warnings
log "backend: tests"
cargo test --workspace --all-targets --locked
log "backend: release build"
cargo build --release --locked
log "backend: complete"
