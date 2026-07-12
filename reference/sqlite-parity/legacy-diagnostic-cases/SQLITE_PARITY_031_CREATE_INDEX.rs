// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_031_CREATE_INDEX

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 31,
        folder: r"SQLITE_PARITY_031_CREATE_INDEX",
        name: r"CREATE_INDEX",
        category: r"SQL_INDEX",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"CREATE INDEX and schema visibility.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b TEXT);
CREATE INDEX idx_t_a ON t(a);
SELECT name,tbl_name FROM sqlite_schema WHERE type='index' AND name='idx_t_a';
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"idx_t_a|t
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
