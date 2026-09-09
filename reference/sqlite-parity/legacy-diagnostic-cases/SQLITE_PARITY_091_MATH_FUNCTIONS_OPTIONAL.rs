// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_091_MATH_FUNCTIONS_OPTIONAL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 91,
        folder: r"SQLITE_PARITY_091_MATH_FUNCTIONS_OPTIONAL",
        name: r"MATH_FUNCTIONS_OPTIONAL",
        category: r"SQL_FUNCTIONS_OPTIONAL",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql",
        description: r"Math functions when compiled/enabled: sin, pow, sqrt, ceil, floor.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT round(sin(0),2), pow(2,3), sqrt(9), ceil(1.2), floor(1.8);
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0.0|8.0|3.0|2.0|1.0
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
