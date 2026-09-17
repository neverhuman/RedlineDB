// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_224_OPT_STATS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 224,
        folder: r"SQLITE_PARITY_224_OPT_STATS",
        name: r"OPT_STATS",
        category: r"CLI_OPTION_DIAGNOSTIC",
        priority: r"P3",
        profile: r"memory",
        kind: r"argv",
        description: r"-stats command-line option emits statistics.",
        status: r"active",
        db: r":memory:",
        args: &[r"-stats", r":memory:", r"SELECT 1;"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"1"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[r"Memory"],
        files: &[],
        script: None,
        notes: r"",
    }
}
