// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_195_OPT_SAFE_MODE_BLOCKS_SHELL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 195,
        folder: r"SQLITE_PARITY_195_OPT_SAFE_MODE_BLOCKS_SHELL",
        name: r"OPT_SAFE_MODE_BLOCKS_SHELL",
        category: r"CLI_OPTION_NEGATIVE",
        priority: r"P2",
        profile: r"memory",
        kind: r"argv",
        description: r"-safe blocks unsafe shell command.",
        status: r"active",
        db: r":memory:",
        args: &[r"-safe", r":memory:", r".shell echo nope"],
        stdin: r"",
        expected_exit: 1,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[r"safe mode"],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
