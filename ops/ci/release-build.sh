#!/usr/bin/env bash
set -euo pipefail

: "${TAG:?TAG is required}"
: "${ARTIFACT:?ARTIFACT is required}"
: "${LIB_NAME:?LIB_NAME is required}"
: "${TARGET:?TARGET is required}"

SOURCE_DIR="${SOURCE_DIR:-.}"
OUTPUT_DIR="${OUTPUT_DIR:-.}"
cd "$SOURCE_DIR"

cargo build --release --locked --target "${TARGET}" -p redlinedb-cli --bin redlinedb-cli
cargo build --release --locked --target "${TARGET}" -p redlinedb-ffi

TARGET_DIR="${CARGO_TARGET_DIR:-target}"
RELEASE_DIR="${TARGET_DIR}/${TARGET}/release"
PKG="redlinedb-${TAG}-${ARTIFACT}"
PKG_DIR="${OUTPUT_DIR}/${PKG}"
mkdir -p "${PKG_DIR}/bin" "${PKG_DIR}/lib" "${PKG_DIR}/include"

cp "${RELEASE_DIR}/redlinedb-cli" "${PKG_DIR}/bin/redlinedb"
if [ -f "${RELEASE_DIR}/${LIB_NAME}" ]; then
  cp "${RELEASE_DIR}/${LIB_NAME}" "${PKG_DIR}/lib/"
fi
cp "${RELEASE_DIR}/libredlinedb.a" "${PKG_DIR}/lib/"
cp "contracts/c-abi/sqlite3.h" "${PKG_DIR}/include/"
cp "contracts/c-abi/redlinedb.h" "${PKG_DIR}/include/"
printf '%s\n' "${TAG}" > "${PKG_DIR}/VERSION"

mkdir -p "${OUTPUT_DIR}"
tar -czf "${OUTPUT_DIR}/${PKG}.tar.gz" -C "${OUTPUT_DIR}" "${PKG}"

# sha256 — Linux has sha256sum, macOS has shasum
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "${OUTPUT_DIR}/${PKG}.tar.gz" > "${OUTPUT_DIR}/${PKG}.tar.gz.sha256"
else
  shasum -a 256 "${OUTPUT_DIR}/${PKG}.tar.gz" > "${OUTPUT_DIR}/${PKG}.tar.gz.sha256"
fi

if [ -n "${GITHUB_ENV:-}" ]; then
  echo "PKG=${PKG}" >> "$GITHUB_ENV"
fi
