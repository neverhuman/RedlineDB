// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_144_DOT_PROMPT

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 144,
        folder: r"SQLITE_PARITY_144_DOT_PROMPT",
        name: r"DOT_PROMPT",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".prompt smoke in batch mode.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".prompt MAIN CONT
SELECT 1;
",
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
