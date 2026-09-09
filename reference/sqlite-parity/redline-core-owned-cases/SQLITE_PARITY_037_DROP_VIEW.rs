// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_037_DROP_VIEW

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 37,
        folder: r"SQLITE_PARITY_037_DROP_VIEW",
        name: r"DROP_VIEW",
        category: r"SQL_DROP",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"DROP VIEW removes view from schema.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE VIEW v AS SELECT 1 AS x;
DROP VIEW v;
SELECT count(*) FROM sqlite_schema WHERE type='view' AND name='v';
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
