// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_222_OPT_ESCAPE_SYMBOL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 222,
        folder: r"SQLITE_PARITY_222_OPT_ESCAPE_SYMBOL",
        name: r"OPT_ESCAPE_SYMBOL",
        category: r"CLI_OPTION",
        priority: r"P3",
        profile: r"memory",
        kind: r"argv",
        description: r"-escape symbol renders control characters with symbolic escapes.",
        status: r"active",
        db: r":memory:",
        args: &[r"-escape", r"symbol", r":memory:", r"SELECT char(10);"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"\n"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
