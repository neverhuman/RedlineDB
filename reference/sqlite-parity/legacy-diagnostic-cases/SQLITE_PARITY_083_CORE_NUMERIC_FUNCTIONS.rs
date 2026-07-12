// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_083_CORE_NUMERIC_FUNCTIONS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 83,
        folder: r"SQLITE_PARITY_083_CORE_NUMERIC_FUNCTIONS",
        name: r"CORE_NUMERIC_FUNCTIONS",
        category: r"SQL_FUNCTIONS",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"abs, round, sign, min/max scalar.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT abs(-3), round(1.234,2), sign(-9), min(5,2,8), max(5,2,8);
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"3|1.23|-1|2|8
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
