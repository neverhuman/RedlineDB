// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_005_UNIQUE_CONSTRAINT_FAILURE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 5,
        folder: r"SQLITE_PARITY_005_UNIQUE_CONSTRAINT_FAILURE",
        name: r"UNIQUE_CONSTRAINT_FAILURE",
        category: r"SQL_CONSTRAINTS_NEGATIVE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"UNIQUE constraint failure behavior and CLI non-zero exit.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x TEXT UNIQUE);
INSERT INTO t VALUES('dup');
INSERT INTO t VALUES('dup');
",
        expected_exit: 1,
        compare_stdout: true,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[r"UNIQUE constraint failed"],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
