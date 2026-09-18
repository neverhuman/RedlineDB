/* Independent UTF-8 ABI consumer: declarations come only from upstream SQLite. */
#define _GNU_SOURCE
#include <sqlite3.h>
#include <dlfcn.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

#define API_LIST(X) \
 X(sqlite3_open) X(sqlite3_close) X(sqlite3_prepare_v2) X(sqlite3_prepare_v3) \
 X(sqlite3_step) X(sqlite3_finalize) X(sqlite3_column_type) \
 X(sqlite3_column_int64) X(sqlite3_libversion) X(sqlite3_sourceid)
#define DECLARE(name) static __typeof__(name) *p_##name;
API_LIST(DECLARE)
#define CHECK(test) do { if (!(test)) { \
 fprintf(stderr, "FAIL line %d: %s\n", __LINE__, #test); return 1; } } while (0)

int main(int argc, char **argv) {
    if (argc != 4) return 125;
    setbuf(stdout, NULL);
    char wanted[PATH_MAX];
    if (!realpath(argv[1], wanted)) return 125;
    void *library = dlopen(wanted, RTLD_NOW | RTLD_LOCAL);
    if (!library) { fprintf(stderr, "%s\n", dlerror()); return 125; }
#define LOAD(name) do { \
    void *symbol = dlsym(library, #name); \
    Dl_info info; char actual[PATH_MAX]; \
    if (!symbol || !dladdr(symbol, &info) || !realpath(info.dli_fname, actual) || \
        strcmp(actual, wanted)) { fprintf(stderr, "wrong/missing symbol: %s\n", #name); return 125; } \
    memcpy(&p_##name, &symbol, sizeof(symbol)); \
} while (0);
    API_LIST(LOAD)
    printf("loaded=%s\nversion=%s\nsourceid=%s\ncase=%s\n", wanted,
           p_sqlite3_libversion(), p_sqlite3_sourceid(), argv[2]);
    sqlite3 *db = NULL;
    CHECK(p_sqlite3_open(argv[3], &db) == SQLITE_OK);
    const char *test = argv[2];
    sqlite3_stmt *stmt = (sqlite3_stmt *)(uintptr_t)1;
    const char *tail = NULL;
    const char *sql = "SELECT 7; SELECT 9";
    int rc;
    if (!strcmp(test, "v3-zero") || !strcmp(test, "v3-persistent")) {
        unsigned flags = !strcmp(test, "v3-zero") ? 0 : SQLITE_PREPARE_PERSISTENT;
        rc = p_sqlite3_prepare_v3(db, sql, -1, flags, &stmt, &tail);
        printf("rc=%d stmt_null=%d tail_matches=%d\n", rc, stmt == NULL, tail == sql + 9);
        CHECK(rc == SQLITE_OK && stmt && tail == sql + 9);
        CHECK(p_sqlite3_step(stmt) == SQLITE_ROW && p_sqlite3_column_int64(stmt, 0) == 7);
    } else if (!strcmp(test, "bounded-guard") || !strcmp(test, "zero-guard")) {
        long page = sysconf(_SC_PAGESIZE);
        CHECK(page > 0);
        char *mapping = mmap(NULL, (size_t)page * 2, PROT_READ | PROT_WRITE,
                             MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        CHECK(mapping != MAP_FAILED);
        CHECK(mprotect(mapping + page, (size_t)page, PROT_NONE) == 0);
        int length = !strcmp(test, "zero-guard") ? 0 : 8;
        char *input = mapping + page - length;
        if (length) memcpy(input, "SELECT 7", 8); /* deliberately no terminator */
        rc = p_sqlite3_prepare_v2(db, input, length, &stmt, &tail);
        printf("rc=%d stmt_null=%d tail_matches=%d\n", rc, stmt == NULL, tail == input + length);
        CHECK(rc == SQLITE_OK && tail == input + length);
        if (!length) CHECK(stmt == NULL);
        else CHECK(stmt && p_sqlite3_step(stmt) == SQLITE_ROW && p_sqlite3_column_int64(stmt, 0) == 7);
        CHECK(munmap(mapping, (size_t)page * 2) == 0);
    } else if (!strcmp(test, "empty-tail")) {
        sql = "  -- comment\n /* empty */ ";
        rc = p_sqlite3_prepare_v2(db, sql, -1, &stmt, &tail);
        printf("rc=%d stmt_null=%d tail_matches=%d\n", rc, stmt == NULL, tail == sql + strlen(sql));
        CHECK(rc == SQLITE_OK && stmt == NULL && tail == sql + strlen(sql));
    } else if (!strcmp(test, "embedded-nul")) {
        static const char input[] = "SELECT 7\0SELECT 9";
        rc = p_sqlite3_prepare_v2(db, input, sizeof(input) - 1, &stmt, &tail);
        printf("rc=%d tail_matches=%d\n", rc, tail == input + 8);
        CHECK(rc == SQLITE_OK && stmt && tail == input + 8);
        CHECK(p_sqlite3_step(stmt) == SQLITE_ROW && p_sqlite3_column_int64(stmt, 0) == 7);
    } else if (!strcmp(test, "error-output")) {
        rc = p_sqlite3_prepare_v2(db, "SELECT FROM", -1, &stmt, &tail);
        printf("rc=%d stmt_null=%d\n", rc, stmt == NULL);
        CHECK(rc == SQLITE_ERROR && stmt == NULL);
    } else if (!strcmp(test, "v2-tail")) {
        rc = p_sqlite3_prepare_v2(db, sql, -1, &stmt, &tail);
        printf("rc=%d tail_matches=%d\n", rc, tail == sql + 9);
        CHECK(rc == SQLITE_OK && stmt && tail == sql + 9);
        CHECK(p_sqlite3_step(stmt) == SQLITE_ROW && p_sqlite3_column_int64(stmt, 0) == 7);
    } else if (!strcmp(test, "type-tags")) {
        int expected[] = {SQLITE_NULL, SQLITE_INTEGER, SQLITE_FLOAT, SQLITE_TEXT, SQLITE_BLOB};
        CHECK(p_sqlite3_prepare_v2(db, "SELECT NULL, 7, 1.5, 'abc', x'0001'", -1, &stmt, NULL) == SQLITE_OK);
        CHECK(p_sqlite3_step(stmt) == SQLITE_ROW);
        int mismatch = 0;
        for (int i = 0; i < 5; ++i) {
            int actual = p_sqlite3_column_type(stmt, i);
            printf("column=%d actual=%d expected=%d\n", i, actual, expected[i]);
            mismatch |= actual != expected[i];
        }
        CHECK(!mismatch);
    } else return 125;
    if (stmt) CHECK(p_sqlite3_finalize(stmt) == SQLITE_OK);
    CHECK(p_sqlite3_close(db) == SQLITE_OK);
    dlclose(library);
    return 0;
}
