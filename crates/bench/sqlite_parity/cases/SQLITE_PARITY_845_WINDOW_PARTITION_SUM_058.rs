// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_845_WINDOW_PARTITION_SUM_058

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 845,
        folder: r"SQLITE_PARITY_845_WINDOW_PARTITION_SUM_058",
        name: r"WINDOW_PARTITION_SUM_058",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_058.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 59), (2, 60), (0, 61), (1, 62), (2, 63), (0, 64), (1, 65), (2, 66), (0, 67);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|61|1|61
0|64|2|125
0|67|3|192
1|59|1|59
1|62|2|121
1|65|3|186
2|60|1|60
2|63|2|123
2|66|3|189
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
