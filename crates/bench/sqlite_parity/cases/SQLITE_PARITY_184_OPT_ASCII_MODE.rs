// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_184_OPT_ASCII_MODE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 184,
        folder: r"SQLITE_PARITY_184_OPT_ASCII_MODE",
        name: r"OPT_ASCII_MODE",
        category: r"CLI_OPTION",
        priority: r"P2",
        profile: r"memory",
        kind: r"argv",
        description: r"-ascii output mode.",
        status: r"active",
        db: r":memory:",
        args: &[r"-ascii", r":memory:", r"SELECT 1,2;"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"12"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
