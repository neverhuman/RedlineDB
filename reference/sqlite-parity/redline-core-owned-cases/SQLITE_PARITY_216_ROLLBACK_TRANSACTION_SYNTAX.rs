// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_216_ROLLBACK_TRANSACTION_SYNTAX

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 216,
        folder: r"SQLITE_PARITY_216_ROLLBACK_TRANSACTION_SYNTAX",
        name: r"ROLLBACK_TRANSACTION_SYNTAX",
        category: r"SQL_TRANSACTION",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"ROLLBACK TRANSACTION syntax.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x);
BEGIN TRANSACTION;
INSERT INTO t VALUES(1);
ROLLBACK TRANSACTION;
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
