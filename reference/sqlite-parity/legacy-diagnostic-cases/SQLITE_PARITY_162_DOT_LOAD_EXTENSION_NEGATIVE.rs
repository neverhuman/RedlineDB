// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_162_DOT_LOAD_EXTENSION_NEGATIVE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 162,
        folder: r"SQLITE_PARITY_162_DOT_LOAD_EXTENSION_NEGATIVE",
        name: r"DOT_LOAD_EXTENSION_NEGATIVE",
        category: r"CLI_SIDE_EFFECT",
        priority: r"P4",
        profile: r"side_effect",
        kind: r"cli",
        description: r".load non-existent extension negative path.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".load {{TMP}}/no_such_extension
",
        expected_exit: 1,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[r"Error"],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
