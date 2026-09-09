// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_221_OPT_DOUBLE_DASH_END_OPTIONS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 221,
        folder: r"SQLITE_PARITY_221_OPT_DOUBLE_DASH_END_OPTIONS",
        name: r"OPT_DOUBLE_DASH_END_OPTIONS",
        category: r"CLI_OPTION",
        priority: r"P2",
        profile: r"memory",
        kind: r"argv",
        description: r"-- ends option parsing.",
        status: r"active",
        db: r":memory:",
        args: &[r"--", r":memory:", r"SELECT 1;"],
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
