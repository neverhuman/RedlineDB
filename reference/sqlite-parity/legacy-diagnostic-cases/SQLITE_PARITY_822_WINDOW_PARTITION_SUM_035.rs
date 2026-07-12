// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_822_WINDOW_PARTITION_SUM_035

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 822,
        folder: r"SQLITE_PARITY_822_WINDOW_PARTITION_SUM_035",
        name: r"WINDOW_PARTITION_SUM_035",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_035.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 36), (2, 37), (0, 38), (1, 39), (2, 40), (0, 41), (1, 42), (2, 43), (0, 44);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|38|1|38
0|41|2|79
0|44|3|123
1|36|1|36
1|39|2|75
1|42|3|117
2|37|1|37
2|40|2|77
2|43|3|120
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
