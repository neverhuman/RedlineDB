#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$root"
all=false
case "${1:-}" in
  --all) all=true; shift ;;
  --help|-h) printf 'Usage: %s [--all] (PREFIX defaults to ~/.local)\n' "$0"; exit 0 ;;
esac
[[ $# == 0 ]] || { printf 'Usage: %s [--all]\n' "$0" >&2; exit 64; }
prefix=${PREFIX:-$HOME/.local}
dest=${JERYU_INSTALL_DIR:-${REDLINEDB_INSTALL_DIR:-$prefix/bin}}
target_dir=${CARGO_TARGET_DIR:-$root/target}
bins=(redlinedb redlinedb-cli redlinedb-server)
if "$all"; then bins+=(redline-testing redline-web redline-proof redlinedb-client-smoke db-shim-parity); fi
for bin in "${bins[@]}"; do
  [[ -x $target_dir/release/$bin ]] || { printf 'missing %s; run ./scripts/build-from-source.sh first\n' "$target_dir/release/$bin" >&2; exit 1; }
done
mkdir -p "$dest" "$prefix/lib" "$prefix/include"
for bin in "${bins[@]}"; do install -m 755 "$target_dir/release/$bin" "$dest/$bin"; done
for lib in "$target_dir"/release/libredlinedb.{a,so,dylib}; do
  if [[ -f $lib ]]; then
    install -m 644 "$lib" "$prefix/lib/"
    if [[ $lib == *.dylib ]]; then
      install_name_tool -id '@rpath/libredlinedb.dylib' "$prefix/lib/libredlinedb.dylib"
      codesign --force --sign - "$prefix/lib/libredlinedb.dylib"
    fi
  fi
done
install -m 644 contracts/c-abi/redlinedb.h contracts/c-abi/sqlite3.h "$prefix/include/"
printf 'Installed binaries in %s, libraries and headers in %s\n' "$dest" "$prefix"
