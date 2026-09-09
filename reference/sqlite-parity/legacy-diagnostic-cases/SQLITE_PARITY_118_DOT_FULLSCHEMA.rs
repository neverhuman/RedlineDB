// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_118_DOT_FULLSCHEMA

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 118,
        folder: r"SQLITE_PARITY_118_DOT_FULLSCHEMA",
        name: r"DOT_FULLSCHEMA",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".fullschema includes schema.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r"CREATE TABLE t(x INT);
.fullschema
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"CREATE TABLE t"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
