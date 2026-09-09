// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_847_WINDOW_PARTITION_SUM_060

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 847,
        folder: r"SQLITE_PARITY_847_WINDOW_PARTITION_SUM_060",
        name: r"WINDOW_PARTITION_SUM_060",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_060.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 61), (2, 62), (0, 63), (1, 64), (2, 65), (0, 66), (1, 67), (2, 68), (0, 69);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|63|1|63
0|66|2|129
0|69|3|198
1|61|1|61
1|64|2|125
1|67|3|192
2|62|1|62
2|65|2|127
2|68|3|195
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
