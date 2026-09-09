// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_080_NOT_NULL_FAILURE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 80,
        folder: r"SQLITE_PARITY_080_NOT_NULL_FAILURE",
        name: r"NOT_NULL_FAILURE",
        category: r"SQL_CONSTRAINTS_NEGATIVE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"NOT NULL constraint failure.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x INT NOT NULL);
INSERT INTO t VALUES(NULL);
",
        expected_exit: 1,
        compare_stdout: true,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[r"NOT NULL constraint failed"],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
