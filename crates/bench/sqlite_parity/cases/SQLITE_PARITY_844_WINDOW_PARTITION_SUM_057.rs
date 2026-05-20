// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_844_WINDOW_PARTITION_SUM_057

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 844,
        folder: r"SQLITE_PARITY_844_WINDOW_PARTITION_SUM_057",
        name: r"WINDOW_PARTITION_SUM_057",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_057.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 58), (2, 59), (0, 60), (1, 61), (2, 62), (0, 63), (1, 64), (2, 65), (0, 66);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|60|1|60
0|63|2|123
0|66|3|189
1|58|1|58
1|61|2|119
1|64|3|183
2|59|1|59
2|62|2|121
2|65|3|186
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
