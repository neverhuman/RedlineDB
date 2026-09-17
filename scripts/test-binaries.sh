#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
bin=$(cd "${1:-$root/target/release}" && pwd)
work=$(mktemp -d)
server_pid='' web_pid=''
cleanup() {
  [[ -z $server_pid ]] || kill "$server_pid" 2>/dev/null || true
  [[ -z $web_pid ]] || kill "$web_pid" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT
cd "$work"
# Independent invocations prove committed data survives close/reopen.
"$bin/redlinedb" -batch -bail "$work/persistent database.redline" 'CREATE TABLE smoke(n INTEGER); BEGIN; INSERT INTO smoke VALUES(42); COMMIT;'
[[ $("$bin/redlinedb" -batch -bail "$work/persistent database.redline" 'SELECT n FROM smoke;') == 42 ]]
port=$((20000 + RANDOM % 20000))
"$bin/redlinedb-server" --database "$work/server database.redline" --listen "127.0.0.1:$port" > "$work/server.log" 2>&1 &
server_pid=$!
ready=false
for _attempt in {1..50}; do
  if (echo > /dev/tcp/127.0.0.1/$port) 2>/dev/null; then ready=true; break; fi
  sleep 0.1
done
"$ready" || { cat "$work/server.log"; exit 1; }
"$bin/redlinedb-client-smoke" "127.0.0.1:$port"
web_port=$((port + 1))
"$bin/redline-web" --target-bin "$bin/redlinedb" --bind "127.0.0.1:$web_port" > "$work/web.log" 2>&1 &
web_pid=$!
ready=false
for _attempt in {1..50}; do
  if curl -fsS "http://127.0.0.1:$web_port/api/health" > /dev/null 2>&1; then ready=true; break; fi
  sleep 0.1
done
"$ready" || { cat "$work/web.log"; exit 1; }
curl -fsS "http://127.0.0.1:$web_port/" | grep -q '<html'
curl -fsS "http://127.0.0.1:$web_port/api/query" -H 'Content-Type: application/json' --data '{"sql":"SELECT 42 AS answer","maxRows":10}' | jq -e '.rows == [[42]]' >/dev/null
if [[ ${CHECK_FFI:-1} == 1 ]]; then
  cat > "$work/ffi.c" <<'C'
#include "redlinedb.h"
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
  lib=${REDLINE_LIB_DIR:-$bin}
  cc "$work/ffi.c" -I "$root/contracts/c-abi" -L "$lib" -Wl,-rpath,"$lib" -lredlinedb -o "$work/ffi-smoke"
  "$work/ffi-smoke"
  printf 'FFI linking smoke passed.\n'
fi
printf 'SQL, persistence, client transactions and embedded web smoke passed.\n'
