// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_079_CHECK_CONSTRAINT_FAILURE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 79,
        folder: r"SQLITE_PARITY_079_CHECK_CONSTRAINT_FAILURE",
        name: r"CHECK_CONSTRAINT_FAILURE",
        category: r"SQL_CONSTRAINTS_NEGATIVE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"CHECK constraint failure.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x INT CHECK(x>0));
INSERT INTO t VALUES(0);
",
        expected_exit: 1,
        compare_stdout: true,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[r"CHECK constraint failed"],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
