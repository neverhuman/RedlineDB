// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_843_WINDOW_PARTITION_SUM_056

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 843,
        folder: r"SQLITE_PARITY_843_WINDOW_PARTITION_SUM_056",
        name: r"WINDOW_PARTITION_SUM_056",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_056.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 57), (2, 58), (0, 59), (1, 60), (2, 61), (0, 62), (1, 63), (2, 64), (0, 65);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|59|1|59
0|62|2|121
0|65|3|186
1|57|1|57
1|60|2|117
1|63|3|180
2|58|1|58
2|61|2|119
2|64|3|183
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
