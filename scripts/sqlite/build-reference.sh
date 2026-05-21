#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

version="3.53.1"
encoded_version="3530100"
archive_name="sqlite-autoconf-${encoded_version}.tar.gz"
archive_sha3="36ca143645cf76997d07b66e9244c636b8ccdec64a1d50558259c4e415e6558b"
download_url="${REDLINEDB_SQLITE_REFERENCE_URL:-https://sqlite.org/2026/${archive_name}}"
prefix="${REDLINEDB_SQLITE_REFERENCE_PREFIX:-$repo_root/target/sqlite-reference/$version}"
bin="$prefix/bin/sqlite3"
stamp="$prefix/.sqlite-reference-sha3"

need_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf '%s is required to build the SQLite reference shell\n' "$1" >&2
    exit 127
  fi
}

archive_digest() {
  openssl dgst -sha3-256 "$1" | awk '{print $2}'
}

existing_shell_is_current() {
  [ -x "$bin" ] || return 1
  [ -f "$stamp" ] || return 1
  [ "$(cat "$stamp")" = "$archive_sha3" ] || return 1
  "$bin" -batch :memory: 'SELECT sqlite_version();' | grep -qx "$version"
}

smoke_reference_shell() {
  local output
  output="$("$bin" -batch :memory: <<'SQL'
CREATE VIRTUAL TABLE ft USING fts5(content);
INSERT INTO ft VALUES('alpha beta');
SELECT highlight(ft,0,'[',']') FROM ft WHERE ft MATCH 'alpha';
CREATE VIRTUAL TABLE rt USING rtree(id,x1,x2,y1,y2);
INSERT INTO rt VALUES(1,0,10,0,10);
SELECT id FROM rt WHERE x1<=5 AND x2>=5;
CREATE TABLE t(a);
INSERT INTO t VALUES(1);
SELECT count(*)>0 FROM dbstat;
SELECT sum(value) FROM generate_series(1,3);
WITH t(x) AS (VALUES('a10'),('a2'))
SELECT group_concat(x, ',') FROM (SELECT x FROM t ORDER BY x COLLATE uint);
SELECT sqrt(9), ceil(1.2), percentile_cont(value,0.5) FROM generate_series(1,3);
SQL
)"
  if [ "$output" != "$(printf '[alpha] beta\n1\n1\n6\na2,a10\n3.0|2.0|2.0')" ]; then
    printf 'SQLite reference shell smoke failed:\n%s\n' "$output" >&2
    exit 1
  fi
}

if existing_shell_is_current; then
  smoke_reference_shell
  printf '%s\n' "$bin"
  exit 0
fi

need_tool awk
need_tool cc
need_tool curl
need_tool grep
need_tool make
need_tool openssl
need_tool tar

downloads_dir="$repo_root/target/sqlite-reference/downloads"
source_parent="$repo_root/target/sqlite-reference/source"
build_dir="$repo_root/target/sqlite-reference/build/$encoded_version"
archive_path="$downloads_dir/$archive_name"
source_dir="$source_parent/sqlite-autoconf-$encoded_version"

mkdir -p "$downloads_dir" "$source_parent" "$(dirname "$build_dir")" "$prefix/bin"

if [ ! -f "$archive_path" ] || [ "$(archive_digest "$archive_path")" != "$archive_sha3" ]; then
  curl -fsSLo "$archive_path" "$download_url"
fi

actual_sha3="$(archive_digest "$archive_path")"
if [ "$actual_sha3" != "$archive_sha3" ]; then
  printf 'SQLite reference archive SHA3 mismatch: expected %s got %s\n' \
    "$archive_sha3" "$actual_sha3" >&2
  exit 1
fi

rm -rf "$source_dir" "$build_dir"
tar -xzf "$archive_path" -C "$source_parent"
mkdir -p "$build_dir"

sqlite_cflags=(
  -O2
  -DSQLITE_ENABLE_BYTECODE_VTAB
  -DSQLITE_ENABLE_COLUMN_METADATA
  -DSQLITE_ENABLE_DBPAGE_VTAB
  -DSQLITE_ENABLE_DBSTAT_VTAB
  -DSQLITE_ENABLE_DESERIALIZE
  -DSQLITE_ENABLE_EXPLAIN_COMMENTS
  -DSQLITE_ENABLE_FTS5
  -DSQLITE_ENABLE_MATH_FUNCTIONS
  -DSQLITE_ENABLE_OFFSET_SQL_FUNC
  -DSQLITE_ENABLE_PERCENTILE
  -DSQLITE_ENABLE_PREUPDATE_HOOK
  -DSQLITE_ENABLE_RTREE
  -DSQLITE_ENABLE_SESSION
  -DSQLITE_ENABLE_STMT_SCANSTATUS
  -DSQLITE_ENABLE_UPDATE_DELETE_LIMIT
  -DSQLITE_ENABLE_UNKNOWN_SQL_FUNCTION
  -DSQLITE_HAVE_ZLIB=1
)
sqlite_ldflags=(-lz -lm -ldl -lpthread)

jobs="${REDLINEDB_SQLITE_REFERENCE_JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '2')}"

(
  cd "$build_dir"
  CFLAGS="${sqlite_cflags[*]}" \
    LDFLAGS="${sqlite_ldflags[*]}" \
    "$source_dir/configure" --disable-shared --enable-static --prefix="$prefix" >&2
  make -j "$jobs" sqlite3 >&2
)

install -m 0755 "$build_dir/sqlite3" "$bin"
"$bin" -batch :memory: 'PRAGMA compile_options;' > "$prefix/compile-options.txt"
printf '%s\n' "$archive_sha3" > "$stamp"
smoke_reference_shell
printf '%s\n' "$bin"
