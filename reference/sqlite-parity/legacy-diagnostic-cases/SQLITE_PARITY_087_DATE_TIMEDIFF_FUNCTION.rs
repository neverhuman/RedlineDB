// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_087_DATE_TIMEDIFF_FUNCTION

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 87,
        folder: r"SQLITE_PARITY_087_DATE_TIMEDIFF_FUNCTION",
        name: r"DATE_TIMEDIFF_FUNCTION",
        category: r"SQL_FUNCTIONS",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"timediff() fixed input shape.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT timediff('2024-01-02','2024-01-01');
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"+0000-00-01 00:00:00.000
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
