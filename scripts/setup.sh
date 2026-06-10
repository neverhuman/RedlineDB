#!/usr/bin/env bash
# One-command setup: install toolchain deps, build the frontend, and build the
# binary (which embeds the built frontend). After this, validate with:
#   bash ops/ci/pr-ci.sh
set -Eeuo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[setup] repository: $ROOT_DIR"

# Wire the mandatory pre-push gate.
if [[ -d .git ]]; then
  git config core.hooksPath ops/git-hooks || true
  echo "[setup] git hooks path -> ops/git-hooks"
fi

if command -v npm >/dev/null 2>&1; then
  echo "[setup] installing frontend deps (apps/web)"
  (cd apps/web && (npm ci --no-audit --no-fund || npm install --no-audit --no-fund))
  echo "[setup] building frontend (apps/web/dist)"
  (cd apps/web && npm run build)
else
  echo "[setup][warn] npm not found; skipping frontend build" >&2
fi

if command -v cargo >/dev/null 2>&1; then
  echo "[setup] building backend (embeds apps/web/dist)"
  cargo build --release --locked
else
  echo "[setup][warn] cargo not found; skipping backend build" >&2
fi

echo "[setup] done. Validate with: bash ops/ci/pr-ci.sh"
