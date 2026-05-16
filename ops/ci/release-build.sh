#!/usr/bin/env bash
set -euo pipefail

cargo build --release --locked --target "${TARGET}" -p redlinedb-cli
cargo build --release --locked --target "${TARGET}" -p redlinedb-ffi

PKG="redlinedb-${TAG}-${ARTIFACT}"
mkdir -p "${PKG}/bin" "${PKG}/lib" "${PKG}/include"

cp "target/${TARGET}/release/redlinedb" "${PKG}/bin/redlinedb"
cp "target/${TARGET}/release/${LIB_NAME}" "${PKG}/lib/" 2>/dev/null || true
cp "target/${TARGET}/release/libredlinedb.a" "${PKG}/lib/"
cp "crates/ffi/include/sqlite3.h" "${PKG}/include/"
cp "contracts/c-abi/redlinedb.h" "${PKG}/include/"

tar -czf "${PKG}.tar.gz" "${PKG}/"

# sha256 — Linux has sha256sum, macOS has shasum
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "${PKG}.tar.gz" > "${PKG}.tar.gz.sha256"
else
  shasum -a 256 "${PKG}.tar.gz" > "${PKG}.tar.gz.sha256"
fi

echo "PKG=${PKG}" >> "$GITHUB_ENV"
