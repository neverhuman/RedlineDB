// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_048_PRAGMA_USER_VERSION

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 48,
        folder: r"SQLITE_PARITY_048_PRAGMA_USER_VERSION",
        name: r"PRAGMA_USER_VERSION",
        category: r"SQL_PRAGMA",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"PRAGMA user_version set/query in memory.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
PRAGMA user_version=42;
PRAGMA user_version;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"42
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
