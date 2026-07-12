// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_827_WINDOW_PARTITION_SUM_040

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 827,
        folder: r"SQLITE_PARITY_827_WINDOW_PARTITION_SUM_040",
        name: r"WINDOW_PARTITION_SUM_040",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_040.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 41), (2, 42), (0, 43), (1, 44), (2, 45), (0, 46), (1, 47), (2, 48), (0, 49);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|43|1|43
0|46|2|89
0|49|3|138
1|41|1|41
1|44|2|85
1|47|3|132
2|42|1|42
2|45|2|87
2|48|3|135
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
