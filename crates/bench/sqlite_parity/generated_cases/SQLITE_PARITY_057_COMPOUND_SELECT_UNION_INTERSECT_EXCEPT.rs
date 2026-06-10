// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_057_COMPOUND_SELECT_UNION_INTERSECT_EXCEPT

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 57,
        folder: r"SQLITE_PARITY_057_COMPOUND_SELECT_UNION_INTERSECT_EXCEPT",
        name: r"COMPOUND_SELECT_UNION_INTERSECT_EXCEPT",
        category: r"SQL_SELECT",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"UNION, UNION ALL, INTERSECT, EXCEPT.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT x FROM (SELECT 1 AS x UNION SELECT 2) INTERSECT SELECT 2;
SELECT x FROM (SELECT 1 AS x UNION ALL SELECT 1) EXCEPT SELECT 2;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"2
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
