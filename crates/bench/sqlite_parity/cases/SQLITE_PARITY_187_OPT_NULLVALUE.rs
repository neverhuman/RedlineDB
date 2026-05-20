// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_187_OPT_NULLVALUE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 187,
        folder: r"SQLITE_PARITY_187_OPT_NULLVALUE",
        name: r"OPT_NULLVALUE",
        category: r"CLI_OPTION",
        priority: r"P1",
        profile: r"memory",
        kind: r"argv",
        description: r"-nullvalue rendering.",
        status: r"active",
        db: r":memory:",
        args: &[r"-nullvalue", r"NULL", r":memory:", r"SELECT NULL,1;"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"NULL|1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
