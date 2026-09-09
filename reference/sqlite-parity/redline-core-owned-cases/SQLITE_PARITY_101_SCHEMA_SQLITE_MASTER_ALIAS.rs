// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_101_SCHEMA_SQLITE_MASTER_ALIAS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 101,
        folder: r"SQLITE_PARITY_101_SCHEMA_SQLITE_MASTER_ALIAS",
        name: r"SCHEMA_SQLITE_MASTER_ALIAS",
        category: r"SQL_SCHEMA",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"sqlite_master compatibility alias.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x INT);
SELECT count(*) FROM sqlite_master WHERE name='t';
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
