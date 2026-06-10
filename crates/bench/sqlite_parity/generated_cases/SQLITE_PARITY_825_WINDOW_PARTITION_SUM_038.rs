// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_825_WINDOW_PARTITION_SUM_038

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 825,
        folder: r"SQLITE_PARITY_825_WINDOW_PARTITION_SUM_038",
        name: r"WINDOW_PARTITION_SUM_038",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_038.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 39), (2, 40), (0, 41), (1, 42), (2, 43), (0, 44), (1, 45), (2, 46), (0, 47);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|41|1|41
0|44|2|85
0|47|3|132
1|39|1|39
1|42|2|81
1|45|3|126
2|40|1|40
2|43|2|83
2|46|3|129
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
