// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_192_OPT_INIT_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 192,
        folder: r"SQLITE_PARITY_192_OPT_INIT_TEMPFILE",
        name: r"OPT_INIT_TEMPFILE",
        category: r"CLI_OPTION_TEMPFILE",
        priority: r"P2",
        profile: r"tempfile",
        kind: r"argv",
        description: r"-init reads temp init script.",
        status: r"active",
        db: r":memory:",
        args: &[r"-init", r"{{TMP}}/init.sql", r":memory:", r"SELECT NULL,1;"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"NULL|1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[crate::FixtureFile { path: r"init.sql", contents: r".mode list
.nullvalue NULL
" }],
        script: None,
        notes: r"",
    }
}
