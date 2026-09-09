// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_071_BETWEEN_IN_ISNULL_IS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 71,
        folder: r"SQLITE_PARITY_071_BETWEEN_IN_ISNULL_IS",
        name: r"BETWEEN_IN_ISNULL_IS",
        category: r"SQL_OPERATORS",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"BETWEEN, IN, IS NULL, IS / IS NOT.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 2 BETWEEN 1 AND 3, 4 IN (1,2,3), NULL IS NULL, NULL = NULL, NULL IS NOT 1;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|0|1|NULL|1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
