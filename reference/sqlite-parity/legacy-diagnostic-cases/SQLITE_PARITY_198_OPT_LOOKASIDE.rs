// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_198_OPT_LOOKASIDE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 198,
        folder: r"SQLITE_PARITY_198_OPT_LOOKASIDE",
        name: r"OPT_LOOKASIDE",
        category: r"CLI_OPTION",
        priority: r"P3",
        profile: r"memory",
        kind: r"argv",
        description: r"-lookaside smoke.",
        status: r"active",
        db: r":memory:",
        args: &[r"-lookaside", r"64", r"4", r":memory:", r"SELECT 1;"],
        stdin: r"",
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
