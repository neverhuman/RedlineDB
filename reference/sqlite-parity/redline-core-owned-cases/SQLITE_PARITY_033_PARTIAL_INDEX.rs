// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_033_PARTIAL_INDEX

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 33,
        folder: r"SQLITE_PARITY_033_PARTIAL_INDEX",
        name: r"PARTIAL_INDEX",
        category: r"SQL_INDEX",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"Partial index WHERE clause stored in schema.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT);
CREATE INDEX idx_partial ON t(a) WHERE b=1;
SELECT sql LIKE '%WHERE b=1%' FROM sqlite_schema WHERE name='idx_partial';
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
