// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_186_OPT_NEWLINE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 186,
        folder: r"SQLITE_PARITY_186_OPT_NEWLINE",
        name: r"OPT_NEWLINE",
        category: r"CLI_OPTION",
        priority: r"P2",
        profile: r"memory",
        kind: r"argv",
        description: r"-newline row separator.",
        status: r"active",
        db: r":memory:",
        args: &[r"-newline", r"@@", r":memory:", r"SELECT 1 UNION ALL SELECT 2;"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1@@2@@"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
