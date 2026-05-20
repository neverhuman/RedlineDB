// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_846_WINDOW_PARTITION_SUM_059

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 846,
        folder: r"SQLITE_PARITY_846_WINDOW_PARTITION_SUM_059",
        name: r"WINDOW_PARTITION_SUM_059",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_059.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 60), (2, 61), (0, 62), (1, 63), (2, 64), (0, 65), (1, 66), (2, 67), (0, 68);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|62|1|62
0|65|2|127
0|68|3|195
1|60|1|60
1|63|2|123
1|66|3|189
2|61|1|61
2|64|2|125
2|67|3|192
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
