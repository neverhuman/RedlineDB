// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_842_WINDOW_PARTITION_SUM_055

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 842,
        folder: r"SQLITE_PARITY_842_WINDOW_PARTITION_SUM_055",
        name: r"WINDOW_PARTITION_SUM_055",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_055.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 56), (2, 57), (0, 58), (1, 59), (2, 60), (0, 61), (1, 62), (2, 63), (0, 64);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|58|1|58
0|61|2|119
0|64|3|183
1|56|1|56
1|59|2|115
1|62|3|177
2|57|1|57
2|60|2|117
2|63|3|180
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
