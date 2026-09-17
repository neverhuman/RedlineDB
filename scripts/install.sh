#!/usr/bin/env bash
# RedlineDB installer — detects platform, downloads the correct release tarball,
# and installs the CLI binary, native library, and C headers.
#
# Usage:
#   curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | bash
#
# Options (env vars):
#   PREFIX   — install root (default: /usr/local)
#   VERSION  — release tag to install (default: latest)
#   REDLINEDB_SHA256 — expected tarball SHA-256 digest (optional hard pin)
#
# Example — install to ~/redlinedb:
#   curl -LsSf ... | PREFIX=~/redlinedb bash
#
# After install:
#   # Rust crate: add to Cargo.toml — redlinedb = "1"
#   # C drop-in:  cc myapp.c -I${PREFIX}/include -L${PREFIX}/lib -lredlinedb -o myapp

set -euo pipefail

REPO="neverhuman/RedlineDB"
PREFIX="${PREFIX:-/usr/local}"
VERSION="${VERSION:-latest}"
REDLINEDB_SHA256="${REDLINEDB_SHA256:-}"

usage() {
  cat <<'USAGE'
usage: scripts/install.sh

Environment:
  PREFIX            install root (default: /usr/local)
  VERSION           release tag to install (default: latest)
  REDLINEDB_SHA256  expected tarball SHA-256 digest (optional hard pin)
USAGE
}

case "${1:-}" in
  -h|--help|help)
    usage
    exit 0
    ;;
  "")
    ;;
  *)
    printf 'error: unknown argument %s\n\n' "$1" >&2
    usage >&2
    exit 64
    ;;
esac

# ── Platform detection ────────────────────────────────────────────────────────

OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}-${ARCH}" in
  Linux-x86_64)   ARTIFACT="linux-x86_64" ;;
  Darwin-arm64)   ARTIFACT="macos-arm64"  ;;
  Darwin-x86_64)  ARTIFACT="macos-x86_64" ;;
  *)
    printf 'error: unsupported platform %s-%s\n' "${OS}" "${ARCH}" >&2
    printf 'Supported: Linux-x86_64, Darwin-arm64, Darwin-x86_64\n' >&2
    exit 1
    ;;
esac

# ── Resolve version ───────────────────────────────────────────────────────────

if [ "${VERSION}" = "latest" ]; then
  printf 'Fetching latest release...\n'
  TAG="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' \
    | sed -E 's/.*"([^"]+)".*/\1/')"
else
  TAG="${VERSION}"
fi

PKG="redlinedb-${TAG}-${ARTIFACT}"
URL="https://github.com/${REPO}/releases/download/${TAG}/${PKG}.tar.gz"
SHA_URL="${URL}.sha256"

# ── Download & verify ─────────────────────────────────────────────────────────

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT

printf 'Downloading redlinedb %s for %s...\n' "${TAG}" "${ARTIFACT}"
curl -fsSL "${URL}"     -o "${TMP}/${PKG}.tar.gz"

if [ -n "${REDLINEDB_SHA256}" ]; then
  EXPECTED_SHA256="${REDLINEDB_SHA256}"
else
  if ! curl -fsSL "${SHA_URL}" -o "${TMP}/${PKG}.tar.gz.sha256"; then
    printf 'error: missing checksum asset %s\n' "${SHA_URL}" >&2
    printf 'Refusing to install an unverified release archive.\n' >&2
    exit 1
  fi
  EXPECTED_SHA256="$(awk 'NR == 1 { print $1 }' "${TMP}/${PKG}.tar.gz.sha256")"
fi

if ! printf '%s\n' "${EXPECTED_SHA256}" | grep -Eq '^[0-9a-fA-F]{64}$'; then
  printf 'error: invalid SHA-256 digest for %s\n' "${PKG}.tar.gz" >&2
  exit 1
fi

printf 'Verifying checksum...\n'
if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL_SHA256="$(sha256sum "${TMP}/${PKG}.tar.gz" | awk '{print $1}')"
else
  ACTUAL_SHA256="$(shasum -a 256 "${TMP}/${PKG}.tar.gz" | awk '{print $1}')"
fi

if [ "${EXPECTED_SHA256}" != "${ACTUAL_SHA256}" ]; then
  printf 'error: checksum mismatch for %s\n' "${PKG}.tar.gz" >&2
  printf 'expected: %s\n' "${EXPECTED_SHA256}" >&2
  printf 'actual:   %s\n' "${ACTUAL_SHA256}" >&2
  exit 1
fi

tar -xzf "${TMP}/${PKG}.tar.gz" -C "${TMP}"

if [ ! -x "${TMP}/${PKG}/bin/redlinedb" ]; then
  printf 'error: release archive is missing bin/redlinedb\n' >&2
  exit 1
fi

# ── Install ───────────────────────────────────────────────────────────────────

printf 'Installing to %s...\n' "${PREFIX}"
mkdir -p "${PREFIX}/bin" "${PREFIX}/lib" "${PREFIX}/include"

install -m 755 "${TMP}/${PKG}/bin/redlinedb"  "${PREFIX}/bin/redlinedb"
cp -f          "${TMP}/${PKG}/lib/"*           "${PREFIX}/lib/"
cp -f          "${TMP}/${PKG}/include/"*       "${PREFIX}/include/"

# ── Done ─────────────────────────────────────────────────────────────────────

printf '\nredlinedb %s installed.\n\n' "${TAG}"
printf '  CLI:     %s/bin/redlinedb\n' "${PREFIX}"
printf '  library: %s/lib/libredlinedb.*\n' "${PREFIX}"
printf '  headers: %s/include/redlinedb.h\n' "${PREFIX}"
printf '           %s/include/sqlite3.h  (SQLite drop-in)\n\n' "${PREFIX}"
printf 'Rust projects — add to Cargo.toml:\n'
printf '  redlinedb = "1"\n\n'
printf 'C/C++ drop-in for SQLite:\n'
printf '  cc myapp.c -I%s/include -L%s/lib -lredlinedb -o myapp\n\n' "${PREFIX}" "${PREFIX}"
