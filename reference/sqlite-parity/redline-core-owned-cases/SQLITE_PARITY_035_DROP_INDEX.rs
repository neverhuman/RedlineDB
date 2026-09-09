// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_035_DROP_INDEX

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 35,
        folder: r"SQLITE_PARITY_035_DROP_INDEX",
        name: r"DROP_INDEX",
        category: r"SQL_DROP",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"DROP INDEX removes index from schema.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT);
CREATE INDEX idx ON t(a);
DROP INDEX idx;
SELECT count(*) FROM sqlite_schema WHERE type='index' AND name='idx';
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
