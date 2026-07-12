// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_085_CORE_RANDOM_SHAPE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 85,
        folder: r"SQLITE_PARITY_085_CORE_RANDOM_SHAPE",
        name: r"CORE_RANDOM_SHAPE",
        category: r"SQL_FUNCTIONS",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"random/randomblob shape without depending on random values.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT typeof(random()), length(randomblob(4));
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"integer|4
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
