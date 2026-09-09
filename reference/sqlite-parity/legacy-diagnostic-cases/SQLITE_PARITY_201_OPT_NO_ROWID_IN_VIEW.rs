// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_201_OPT_NO_ROWID_IN_VIEW

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 201,
        folder: r"SQLITE_PARITY_201_OPT_NO_ROWID_IN_VIEW",
        name: r"OPT_NO_ROWID_IN_VIEW",
        category: r"CLI_OPTION",
        priority: r"P4",
        profile: r"memory",
        kind: r"argv",
        description: r"-no-rowid-in-view smoke.",
        status: r"active",
        db: r":memory:",
        args: &[r"-no-rowid-in-view", r":memory:", r"CREATE VIEW v AS SELECT 1 AS x; SELECT x FROM v;"],
        stdin: r"",
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
