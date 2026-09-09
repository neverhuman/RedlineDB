// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_065_CTE_RECURSIVE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 65,
        folder: r"SQLITE_PARITY_065_CTE_RECURSIVE",
        name: r"CTE_RECURSIVE",
        category: r"SQL_CTE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"WITH RECURSIVE sequence generation.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
WITH RECURSIVE seq(x) AS (
  VALUES(1)
  UNION ALL
  SELECT x+1 FROM seq WHERE x<4
)
SELECT group_concat(x,'') FROM seq;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1234
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
