// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_128_DOT_DBCONFIG

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 128,
        folder: r"SQLITE_PARITY_128_DOT_DBCONFIG",
        name: r"DOT_DBCONFIG",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".dbconfig set/query smoke.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".dbconfig defensive on
.dbconfig defensive
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"defensive"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
