#include "sqlite3.h"
#include <string.h>
int main(int argc, char **argv) {
    sqlite3 *db = 0;
    sqlite3_stmt *stmt = 0;
    if (argc != 2 || strcmp(sqlite3_sourceid(), argv[1]) ||
        sqlite3_libversion_number() != SQLITE_VERSION_NUMBER ||
        strcmp(sqlite3_libversion(), SQLITE_VERSION)) return 1;
    if (sqlite3_open(":memory:", &db) != SQLITE_OK) return 2;
    if (sqlite3_prepare_v3(db, "SELECT 42", -1, SQLITE_PREPARE_PERSISTENT,
                           &stmt, 0) != SQLITE_OK) return 3;
    if (sqlite3_step(stmt) != SQLITE_ROW || sqlite3_column_int(stmt, 0) != 42) return 4;
    if (sqlite3_finalize(stmt) != SQLITE_OK) return 5;
    if (sqlite3_exec(db, "CREATE TABLE m(a INTEGER)", 0, 0, 0) != SQLITE_OK) return 6;
    const char *type = 0;
    if (sqlite3_table_column_metadata(db, "main", "m", "a", &type, 0, 0, 0, 0)
        != SQLITE_OK || !type || strcmp(type, "INTEGER")) return 7;
    return sqlite3_close(db) != SQLITE_OK;
}
