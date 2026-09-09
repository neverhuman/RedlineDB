// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_104_SELECT_DISTINCT

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 104,
        folder: r"SQLITE_PARITY_104_SELECT_DISTINCT",
        name: r"SELECT_DISTINCT",
        category: r"SQL_SELECT",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"SELECT DISTINCT duplicate elimination.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
WITH t(x) AS (VALUES(1),(1),(2))
SELECT DISTINCT x FROM t ORDER BY x;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1
2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
