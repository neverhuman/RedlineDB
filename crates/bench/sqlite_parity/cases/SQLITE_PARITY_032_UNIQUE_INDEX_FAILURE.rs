// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_032_UNIQUE_INDEX_FAILURE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 32,
        folder: r"SQLITE_PARITY_032_UNIQUE_INDEX_FAILURE",
        name: r"UNIQUE_INDEX_FAILURE",
        category: r"SQL_INDEX_NEGATIVE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"UNIQUE INDEX enforcement failure.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT);
CREATE UNIQUE INDEX ux_t_a ON t(a);
INSERT INTO t VALUES(1),(1);
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
