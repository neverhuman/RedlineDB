// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_178_OPT_HTML_MODE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 178,
        folder: r"SQLITE_PARITY_178_OPT_HTML_MODE",
        name: r"OPT_HTML_MODE",
        category: r"CLI_OPTION",
        priority: r"P2",
        profile: r"memory",
        kind: r"argv",
        description: r"-html output mode.",
        status: r"active",
        db: r":memory:",
        args: &[r"-html", r":memory:", r"SELECT 1 AS a, 'x' AS b;"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"<TR>", r"<TD>1</TD>"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
