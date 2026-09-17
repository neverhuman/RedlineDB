#ifndef SQLITE3_H
#define SQLITE3_H

/* SQLite-compatible alias shim: existing SQLite consumers (rusqlite, sqlx,
 * Python `sqlite3`, ...) `#include <sqlite3.h>` and link the RedlineDB C ABI
 * without source changes. The canonical C ABI surface lives next to this file
 * in `contracts/c-abi/redlinedb.h`; this header is a thin re-include so the
 * full C ABI lives in one cell. Hand-authored, not generated. */
#include "redlinedb.h"

#endif
