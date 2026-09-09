// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_135_DOT_PROGRESS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 135,
        folder: r"SQLITE_PARITY_135_DOT_PROGRESS",
        name: r"DOT_PROGRESS",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".progress handler smoke.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".progress 1
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
