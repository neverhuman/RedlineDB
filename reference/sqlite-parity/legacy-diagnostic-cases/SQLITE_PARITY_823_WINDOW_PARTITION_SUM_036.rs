// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_823_WINDOW_PARTITION_SUM_036

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 823,
        folder: r"SQLITE_PARITY_823_WINDOW_PARTITION_SUM_036",
        name: r"WINDOW_PARTITION_SUM_036",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_036.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 37), (2, 38), (0, 39), (1, 40), (2, 41), (0, 42), (1, 43), (2, 44), (0, 45);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|39|1|39
0|42|2|81
0|45|3|126
1|37|1|37
1|40|2|77
1|43|3|120
2|38|1|38
2|41|2|79
2|44|3|123
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
