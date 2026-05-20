// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_028_ALTER_TABLE_RENAME

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 28,
        folder: r"SQLITE_PARITY_028_ALTER_TABLE_RENAME",
        name: r"ALTER_TABLE_RENAME",
        category: r"SQL_ALTER",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"ALTER TABLE RENAME TO.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT);
ALTER TABLE t RENAME TO u;
SELECT name FROM sqlite_schema WHERE type='table' AND name='u';
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"u
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
