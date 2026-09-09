// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_214_DROP_TABLE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 214,
        folder: r"SQLITE_PARITY_214_DROP_TABLE",
        name: r"DROP_TABLE",
        category: r"SQL_DROP",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"DROP TABLE removes table from schema.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x INT);
DROP TABLE t;
SELECT count(*) FROM sqlite_schema WHERE type='table' AND name='t';
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
