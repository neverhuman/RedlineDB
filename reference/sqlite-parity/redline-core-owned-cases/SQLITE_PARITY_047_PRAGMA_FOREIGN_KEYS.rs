// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_047_PRAGMA_FOREIGN_KEYS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 47,
        folder: r"SQLITE_PARITY_047_PRAGMA_FOREIGN_KEYS",
        name: r"PRAGMA_FOREIGN_KEYS",
        category: r"SQL_PRAGMA",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"PRAGMA foreign_keys set/query.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
PRAGMA foreign_keys=ON;
PRAGMA foreign_keys;
",
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
