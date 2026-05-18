#ifndef REDLINEDB_H
#define REDLINEDB_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct rldb rldb;
typedef struct rldb_stmt rldb_stmt;
typedef struct rldb_backup rldb_backup;
typedef rldb sqlite3;
typedef rldb_stmt sqlite3_stmt;
typedef rldb_backup sqlite3_backup;

#define RLDB_OK 0
#define RLDB_ERROR 1
#define RLDB_INTERNAL 2
#define RLDB_BUSY 5
#define RLDB_LOCKED 6
#define RLDB_NOMEM 7
#define RLDB_READONLY 8
#define RLDB_INTERRUPT 9
#define RLDB_IOERR 10
#define RLDB_CORRUPT 11
#define RLDB_SCHEMA 17
#define RLDB_TOOBIG 18
#define RLDB_CONSTRAINT 19
#define RLDB_MISMATCH 20
#define RLDB_MISUSE 21
#define RLDB_RANGE 25
#define RLDB_NOTADB 26
#define RLDB_ROW 100
#define RLDB_DONE 101

#define SQLITE_OK 0
#define SQLITE_ERROR 1
#define SQLITE_INTERNAL 2
#define SQLITE_BUSY 5
#define SQLITE_LOCKED 6
#define SQLITE_NOMEM 7
#define SQLITE_READONLY 8
#define SQLITE_INTERRUPT 9
#define SQLITE_IOERR 10
#define SQLITE_CORRUPT 11
#define SQLITE_CANTOPEN 14
#define SQLITE_SCHEMA 17
#define SQLITE_TOOBIG 18
#define SQLITE_CONSTRAINT 19
#define SQLITE_MISMATCH 20
#define SQLITE_MISUSE 21
#define SQLITE_RANGE 25
#define SQLITE_NOTADB 26
#define SQLITE_ROW 100
#define SQLITE_DONE 101

#define RLDB_NULL 0
#define RLDB_INTEGER 1
#define RLDB_REAL 2
#define RLDB_TEXT 3
#define RLDB_BLOB 4

#define SQLITE_NULL 0
#define SQLITE_INTEGER 1
#define SQLITE_FLOAT 2
#define SQLITE_TEXT 3
#define SQLITE_BLOB 4

#define SQLITE_OPEN_READONLY 0x00000001
#define SQLITE_OPEN_READWRITE 0x00000002
#define SQLITE_OPEN_CREATE 0x00000004

typedef struct rldb_config {
    uint32_t struct_size;
    uint32_t flags;
    uint32_t durability;
    uint64_t cache_bytes;
    uint64_t work_mem_bytes;
    uint64_t max_spill_bytes;
    uint32_t statement_cache_capacity;
    uint32_t busy_timeout_ms;
} rldb_config;

int rldb_open(const char *path, rldb **out_db);
int rldb_open_v2(const char *path, const rldb_config *config, rldb **out_db);
int rldb_close(rldb *db);
int rldb_close_v2(rldb *db);
const char *sqlite3_libversion(void);
int sqlite3_libversion_number(void);
const char *sqlite3_sourceid(void);
int sqlite3_threadsafe(void);
const char *sqlite3_errstr(int code);

int rldb_prepare_v2(rldb *db, const char *sql, int nbytes, rldb_stmt **out_stmt, const char **tail);
int sqlite3_prepare_v3(sqlite3 *db, const char *sql, int nbytes, sqlite3_stmt **out_stmt, const char **tail, int flags);
int rldb_step(rldb_stmt *stmt);
int rldb_reset(rldb_stmt *stmt);
int rldb_finalize(rldb_stmt *stmt);
int rldb_clear_bindings(rldb_stmt *stmt);
int sqlite3_stmt_readonly(sqlite3_stmt *stmt);
int sqlite3_stmt_busy(sqlite3_stmt *stmt);
const char *sqlite3_sql(sqlite3_stmt *stmt);

int rldb_bind_null(rldb_stmt *stmt, int index);
int rldb_bind_int64(rldb_stmt *stmt, int index, int64_t value);
int rldb_bind_double(rldb_stmt *stmt, int index, double value);
int rldb_bind_text(rldb_stmt *stmt, int index, const char *value, int nbytes);
int rldb_bind_blob(rldb_stmt *stmt, int index, const void *value, int nbytes);

int rldb_parameter_count(rldb_stmt *stmt);
int rldb_bind_parameter_index(rldb_stmt *stmt, const char *name);

int rldb_column_count(rldb_stmt *stmt);
const char *rldb_column_name(rldb_stmt *stmt, int index);
int rldb_column_type(rldb_stmt *stmt, int index);
int64_t rldb_column_int64(rldb_stmt *stmt, int index);
double rldb_column_double(rldb_stmt *stmt, int index);
const unsigned char *rldb_column_text(rldb_stmt *stmt, int index);
const void *rldb_column_blob(rldb_stmt *stmt, int index);
int rldb_column_bytes(rldb_stmt *stmt, int index);

typedef int (*rldb_exec_callback)(void *, int, char **, char **);
int rldb_exec(rldb *db, const char *sql, rldb_exec_callback callback, void *ctx, char **errmsg);

int rldb_errcode(rldb *db);
const char *rldb_errmsg(rldb *db);
void rldb_free(void *ptr);
void rldb_interrupt(rldb *db);
int rldb_busy_timeout(rldb *db, int milliseconds);
int rldb_changes(rldb *db);
int64_t rldb_last_insert_rowid(rldb *db);

int rldb_checkpoint(rldb *db);
int rldb_vacuum(rldb *db);
int rldb_stats_json(rldb *db, char **out_json);

int rldb_backup_init(rldb *src, const char *dst_path, const rldb_config *dst_config, rldb_backup **out);
int rldb_backup_step(rldb_backup *backup, int batches);
int rldb_backup_finish(rldb_backup *backup);
int rldb_backup_close(rldb_backup *backup);
int rldb_backup_remaining(rldb_backup *backup);
int rldb_backup_pagecount(rldb_backup *backup);

typedef int (*sqlite3_exec_callback)(void *, int, char **, char **);
int sqlite3_open(const char *path, sqlite3 **out_db);
int sqlite3_open_v2(const char *path, sqlite3 **out_db, int flags, const char *vfs);
int sqlite3_close(sqlite3 *db);
int sqlite3_close_v2(sqlite3 *db);

int sqlite3_prepare_v2(sqlite3 *db, const char *sql, int nbytes, sqlite3_stmt **out_stmt, const char **tail);
int sqlite3_step(sqlite3_stmt *stmt);
int sqlite3_reset(sqlite3_stmt *stmt);
int sqlite3_finalize(sqlite3_stmt *stmt);
int sqlite3_clear_bindings(sqlite3_stmt *stmt);

int sqlite3_bind_null(sqlite3_stmt *stmt, int index);
int sqlite3_bind_int64(sqlite3_stmt *stmt, int index, int64_t value);
int sqlite3_bind_double(sqlite3_stmt *stmt, int index, double value);
int sqlite3_bind_text(sqlite3_stmt *stmt, int index, const char *value, int nbytes, void (*destructor)(void *));
int sqlite3_bind_blob(sqlite3_stmt *stmt, int index, const void *value, int nbytes, void (*destructor)(void *));

int sqlite3_parameter_count(sqlite3_stmt *stmt);
int sqlite3_bind_parameter_index(sqlite3_stmt *stmt, const char *name);

int sqlite3_column_count(sqlite3_stmt *stmt);
const char *sqlite3_column_name(sqlite3_stmt *stmt, int index);
int sqlite3_column_type(sqlite3_stmt *stmt, int index);
int64_t sqlite3_column_int64(sqlite3_stmt *stmt, int index);
double sqlite3_column_double(sqlite3_stmt *stmt, int index);
const unsigned char *sqlite3_column_text(sqlite3_stmt *stmt, int index);
const void *sqlite3_column_blob(sqlite3_stmt *stmt, int index);
int sqlite3_column_bytes(sqlite3_stmt *stmt, int index);

int sqlite3_exec(sqlite3 *db, const char *sql, sqlite3_exec_callback callback, void *ctx, char **errmsg);

int sqlite3_errcode(sqlite3 *db);
const char *sqlite3_errmsg(sqlite3 *db);
void sqlite3_free(void *ptr);
void sqlite3_interrupt(sqlite3 *db);
int sqlite3_busy_timeout(sqlite3 *db, int milliseconds);
int sqlite3_extended_result_codes(sqlite3 *db, int onoff);
int sqlite3_changes(sqlite3 *db);
int64_t sqlite3_changes64(sqlite3 *db);
int sqlite3_total_changes(sqlite3 *db);
int64_t sqlite3_total_changes64(sqlite3 *db);
int sqlite3_get_autocommit(sqlite3 *db);
int64_t sqlite3_last_insert_rowid(sqlite3 *db);
sqlite3 *sqlite3_db_handle(sqlite3_stmt *stmt);
const char *sqlite3_db_filename(sqlite3 *db, const char *name);
int sqlite3_db_readonly(sqlite3 *db, const char *name);
int sqlite3_checkpoint(sqlite3 *db);
int sqlite3_vacuum(sqlite3 *db);
int sqlite3_stats_json(sqlite3 *db, char **out_json);

#ifdef __cplusplus
}
#endif

#endif
