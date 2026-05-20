#!/usr/bin/env bash
set -euo pipefail

: "${TAG:?TAG is required}"
: "${ARTIFACT:?ARTIFACT is required}"
: "${LIB_NAME:?LIB_NAME is required}"
: "${TARGET:?TARGET is required}"

SOURCE_DIR="${SOURCE_DIR:-.}"
cd "$SOURCE_DIR"

cargo build --release --locked --target "${TARGET}" -p redlinedb-cli --bin redlinedb-cli
cargo build --release --locked --target "${TARGET}" -p redlinedb-ffi

PKG="redlinedb-${TAG}-${ARTIFACT}"
mkdir -p "${PKG}/bin" "${PKG}/lib" "${PKG}/include"

cp "target/${TARGET}/release/redlinedb-cli" "${PKG}/bin/redlinedb"
if [ -f "target/${TARGET}/release/${LIB_NAME}" ]; then
  cp "target/${TARGET}/release/${LIB_NAME}" "${PKG}/lib/"
fi
cp "target/${TARGET}/release/libredlinedb.a" "${PKG}/lib/"
cp "crates/ffi/include/sqlite3.h" "${PKG}/include/"
cp "contracts/c-abi/redlinedb.h" "${PKG}/include/"
printf '%s\n' "${TAG}" > "${PKG}/VERSION"

tar -czf "${PKG}.tar.gz" "${PKG}/"

# sha256 — Linux has sha256sum, macOS has shasum
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "${PKG}.tar.gz" > "${PKG}.tar.gz.sha256"
else
  shasum -a 256 "${PKG}.tar.gz" > "${PKG}.tar.gz.sha256"
fi

if [ -n "${GITHUB_ENV:-}" ]; then
  echo "PKG=${PKG}" >> "$GITHUB_ENV"
fi
