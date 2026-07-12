// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_820_WINDOW_PARTITION_SUM_033

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 820,
        folder: r"SQLITE_PARITY_820_WINDOW_PARTITION_SUM_033",
        name: r"WINDOW_PARTITION_SUM_033",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_033.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 34), (2, 35), (0, 36), (1, 37), (2, 38), (0, 39), (1, 40), (2, 41), (0, 42);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|36|1|36
0|39|2|75
0|42|3|117
1|34|1|34
1|37|2|71
1|40|3|111
2|35|1|35
2|38|2|73
2|41|3|114
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
