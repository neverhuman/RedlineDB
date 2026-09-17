#!/usr/bin/env bash
# Link a C consumer against installed headers/libraries, then relocate its tree.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
packages=${1:-${OUTPUT_DIR:-$root/target/packages}}
archives=("$packages"/redlinedb-*.tar.gz)
[[ ${#archives[@]} == 1 && -f ${archives[0]} ]] || { echo 'expected one native core archive' >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
prefix="$work/install with spaces"
mkdir -p "$prefix"
tar -xzf "${archives[0]}" -C "$prefix"
case "$(uname -s)" in
  Darwin)
    identity=$(otool -D "$prefix/lib/libredlinedb.dylib" | sed -n '2p')
    case "$identity" in /*) echo "packaged dylib has an absolute load identity: $identity" >&2; exit 1 ;; esac
    codesign --verify --strict "$prefix/lib/libredlinedb.dylib"
    rpath='@loader_path/../lib'
    native_libs=(-liconv -lSystem -lresolv)
    ;;
  Linux)
    # The dynamic loader expands ORIGIN when the relocated consumer runs.
    # shellcheck disable=SC2016
    rpath='$ORIGIN/../lib'
    native_libs=(-ldl -lpthread -lm)
    ;;
  *) echo 'unsupported FFI test platform' >&2; exit 1 ;;
esac
cat > "$work/consumer.c" <<'C'
#include "sqlite3.h"
int main(void) {
    sqlite3 *db = 0;
    sqlite3_stmt *stmt = 0;
    if (sqlite3_open(":memory:", &db) != SQLITE_OK) return 1;
    if (sqlite3_prepare_v2(db, "SELECT 42", -1, &stmt, 0) != SQLITE_OK) return 2;
    if (sqlite3_step(stmt) != SQLITE_ROW || sqlite3_column_int64(stmt, 0) != 42) return 3;
    sqlite3_finalize(stmt);
    return sqlite3_close(db);
}
C
cc "$work/consumer.c" -I "$prefix/include" -L "$prefix/lib" -Wl,-rpath,"$rpath" -lredlinedb -o "$prefix/bin/ffi-dynamic"
cc "$work/consumer.c" -I "$prefix/include" "$prefix/lib/libredlinedb.a" "${native_libs[@]}" -o "$prefix/bin/ffi-static"
mv "$prefix" "$work/relocated install"
cd "$work"
"$work/relocated install/bin/ffi-dynamic"
"$work/relocated install/bin/ffi-static"
printf 'Packaged dynamic/static FFI consumers passed after relocation.\n'
