// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_172_OPT_CMD

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 172,
        folder: r"SQLITE_PARITY_172_OPT_CMD",
        name: r"OPT_CMD",
        category: r"CLI_OPTION",
        priority: r"P1",
        profile: r"memory",
        kind: r"argv",
        description: r"-cmd runs command before input.",
        status: r"active",
        db: r":memory:",
        args: &[r"-cmd", r".mode list", r":memory:", r"SELECT 1;"],
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
