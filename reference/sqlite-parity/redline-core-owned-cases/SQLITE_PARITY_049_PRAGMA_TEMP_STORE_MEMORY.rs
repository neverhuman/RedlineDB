// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_049_PRAGMA_TEMP_STORE_MEMORY

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 49,
        folder: r"SQLITE_PARITY_049_PRAGMA_TEMP_STORE_MEMORY",
        name: r"PRAGMA_TEMP_STORE_MEMORY",
        category: r"SQL_PRAGMA",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"PRAGMA temp_store MEMORY.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
PRAGMA temp_store=MEMORY;
PRAGMA temp_store;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
