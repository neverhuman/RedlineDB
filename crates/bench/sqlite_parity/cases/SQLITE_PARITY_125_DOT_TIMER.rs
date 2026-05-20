// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_125_DOT_TIMER

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 125,
        folder: r"SQLITE_PARITY_125_DOT_TIMER",
        name: r"DOT_TIMER",
        category: r"CLI_DOT_COMMAND_DIAGNOSTIC",
        priority: r"P3",
        profile: r"memory",
        kind: r"cli",
        description: r".timer on emits timing diagnostics.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".timer on
SELECT 1;
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"1"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
