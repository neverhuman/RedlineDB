// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_092_PERCENTILE_FUNCTIONS_OPTIONAL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 92,
        folder: r"SQLITE_PARITY_092_PERCENTILE_FUNCTIONS_OPTIONAL",
        name: r"PERCENTILE_FUNCTIONS_OPTIONAL",
        category: r"SQL_FUNCTIONS_OPTIONAL",
        priority: r"P3",
        profile: r"memory",
        kind: r"sql",
        description: r"Percentile/median aggregate extension when compiled/enabled.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
WITH t(x) AS (VALUES(1),(2),(3))
SELECT median(x), percentile_cont(x,0.5) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"2.0|2.0
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
