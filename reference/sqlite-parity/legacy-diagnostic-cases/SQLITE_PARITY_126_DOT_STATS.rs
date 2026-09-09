// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_126_DOT_STATS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 126,
        folder: r"SQLITE_PARITY_126_DOT_STATS",
        name: r"DOT_STATS",
        category: r"CLI_DOT_COMMAND_DIAGNOSTIC",
        priority: r"P3",
        profile: r"memory",
        kind: r"cli",
        description: r".stats on emits runtime statistics.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".stats on
SELECT 1;
",
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
