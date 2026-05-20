// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_841_WINDOW_PARTITION_SUM_054

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 841,
        folder: r"SQLITE_PARITY_841_WINDOW_PARTITION_SUM_054",
        name: r"WINDOW_PARTITION_SUM_054",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_054.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 55), (2, 56), (0, 57), (1, 58), (2, 59), (0, 60), (1, 61), (2, 62), (0, 63);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|57|1|57
0|60|2|117
0|63|3|180
1|55|1|55
1|58|2|113
1|61|3|174
2|56|1|56
2|59|2|115
2|62|3|177
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
