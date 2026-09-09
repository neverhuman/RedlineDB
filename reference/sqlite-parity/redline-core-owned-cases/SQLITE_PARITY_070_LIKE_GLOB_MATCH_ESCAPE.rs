// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_070_LIKE_GLOB_MATCH_ESCAPE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 70,
        folder: r"SQLITE_PARITY_070_LIKE_GLOB_MATCH_ESCAPE",
        name: r"LIKE_GLOB_MATCH_ESCAPE",
        category: r"SQL_OPERATORS",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"LIKE, GLOB, MATCH-style ESCAPE for LIKE.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 'abc' LIKE 'a%', 'abc' GLOB 'a*', 'a_%' LIKE 'a\_\%' ESCAPE '\';
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|1|1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
