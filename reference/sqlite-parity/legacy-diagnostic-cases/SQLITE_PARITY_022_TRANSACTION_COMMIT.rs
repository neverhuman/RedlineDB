// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_022_TRANSACTION_COMMIT

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 22,
        folder: r"SQLITE_PARITY_022_TRANSACTION_COMMIT",
        name: r"TRANSACTION_COMMIT",
        category: r"SQL_TRANSACTION",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"BEGIN/COMMIT transaction persists changes inside connection.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
BEGIN;
CREATE TABLE t(x INT);
INSERT INTO t VALUES(1);
COMMIT;
SELECT count(*) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
