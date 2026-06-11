#!/usr/bin/env bash
# Build, package, and publish the RedlineDB binary.
# Called from .github/workflows/release.yml after toolchains are set up.
#
# Required env vars (set by workflow):
#   TAG                — the release tag (e.g. v1.2.3)
#   TARGET             — platform target string (e.g. linux-x86_64)
#   CORE_REF           — redline-core git ref that was built
#   GITHUB_REPOSITORY  — hub repo (e.g. neverhuman/RedlineDB)
#   GH_TOKEN           — GitHub token with contents:write
#
# Optional:
#   BUILD_DIR          — path to the checked-out engine (default: engine)
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"

TAG="${TAG:?TAG env var is required}"
TARGET="${TARGET:?TARGET env var is required}"
REPO="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY env var is required}"
CORE_REF="${CORE_REF:-$TAG}"
BUILD_DIR="${BUILD_DIR:-engine}"

# ── Build ────────────────────────────────────────────────────────────────────
log_step "build: cargo build --release --locked (in $BUILD_DIR)"
cd "$BUILD_DIR"
cargo build --release --locked -p cli 2>/dev/null || cargo build --release --locked
cd - >/dev/null

# ── Package ──────────────────────────────────────────────────────────────────
log_step "package: $TAG-$TARGET"
bin="$(find "$BUILD_DIR/target/release" -maxdepth 1 -type f -name redline -perm -u+x | head -1)"
[[ -n "$bin" ]] || { echo "ERROR: redline binary not found under $BUILD_DIR/target/release" >&2; exit 1; }

dist="redline-$TAG-$TARGET"
mkdir -p "$dist"
cp "$bin" "$dist/redline"
cp "$BUILD_DIR/LICENSE" "$dist/" 2>/dev/null || true
tar -czf "$dist.tar.gz" "$dist"
sha256sum "$dist.tar.gz" > "$dist.tar.gz.sha256"
log_ok "packaged $dist.tar.gz"

# ── Publish ──────────────────────────────────────────────────────────────────
log_step "publish: create/update release $TAG"
if ! gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1; then
    gh release create "$TAG" --repo "$REPO" --verify-tag \
        --title "$TAG" \
        --notes "RedlineDB $TAG — built from redline-core@$CORE_REF"
    log_ok "created release $TAG"
fi

artifact="$dist.tar.gz"
existing="$(gh release view "$TAG" --repo "$REPO" --json assets --jq '[.assets[].name]' 2>/dev/null || echo '[]')"
if ! echo "$existing" | grep -qF "\"$(basename "$artifact")\""; then
    gh release upload "$TAG" --repo "$REPO" "$artifact" "$artifact.sha256"
    log_ok "uploaded $(basename "$artifact")"
else
    log_ok "asset already present, skipping upload"
fi
