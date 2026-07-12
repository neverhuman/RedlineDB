// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_067_CASE_COALESCE_NULLIF_IIF

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 67,
        folder: r"SQLITE_PARITY_067_CASE_COALESCE_NULLIF_IIF",
        name: r"CASE_COALESCE_NULLIF_IIF",
        category: r"SQL_EXPRESSIONS",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"CASE, coalesce, ifnull, nullif, iif/if spelling.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT CASE WHEN 1 THEN 'yes' ELSE 'no' END,
       coalesce(NULL,'c'), ifnull(NULL,'i'), nullif(1,1), iif(0,'bad','ok');
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"yes|c|i|NULL|ok
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
