// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_105_CASE_SENSITIVE_LIKE_PRAGMA

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 105,
        folder: r"SQLITE_PARITY_105_CASE_SENSITIVE_LIKE_PRAGMA",
        name: r"CASE_SENSITIVE_LIKE_PRAGMA",
        category: r"SQL_PRAGMA",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql",
        description: r"PRAGMA case_sensitive_like toggle.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
PRAGMA case_sensitive_like=ON;
SELECT 'A' LIKE 'a';
PRAGMA case_sensitive_like=OFF;
SELECT 'A' LIKE 'a';
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0
1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
