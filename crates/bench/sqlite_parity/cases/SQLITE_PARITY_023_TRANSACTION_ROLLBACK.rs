// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_023_TRANSACTION_ROLLBACK

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 23,
        folder: r"SQLITE_PARITY_023_TRANSACTION_ROLLBACK",
        name: r"TRANSACTION_ROLLBACK",
        category: r"SQL_TRANSACTION",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"ROLLBACK removes transactional changes.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x INT);
BEGIN;
INSERT INTO t VALUES(1);
ROLLBACK;
SELECT count(*) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
