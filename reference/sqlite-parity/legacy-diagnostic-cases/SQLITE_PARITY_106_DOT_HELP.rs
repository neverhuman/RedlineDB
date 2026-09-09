// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_106_DOT_HELP

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 106,
        folder: r"SQLITE_PARITY_106_DOT_HELP",
        name: r"DOT_HELP",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".help command list smoke check.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".help
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r".mode", r".schema", r".quit"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
