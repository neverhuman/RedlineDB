// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_034_EXPRESSION_INDEX

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 34,
        folder: r"SQLITE_PARITY_034_EXPRESSION_INDEX",
        name: r"EXPRESSION_INDEX",
        category: r"SQL_INDEX",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"Expression index on lower(name).",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(name TEXT);
CREATE INDEX idx_expr ON t(lower(name));
SELECT sql LIKE '%lower(name)%' FROM sqlite_schema WHERE name='idx_expr';
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
