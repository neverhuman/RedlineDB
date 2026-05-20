// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_849_WINDOW_PARTITION_SUM_062

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 849,
        folder: r"SQLITE_PARITY_849_WINDOW_PARTITION_SUM_062",
        name: r"WINDOW_PARTITION_SUM_062",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_062.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 63), (2, 64), (0, 65), (1, 66), (2, 67), (0, 68), (1, 69), (2, 70), (0, 71);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|65|1|65
0|68|2|133
0|71|3|204
1|63|1|63
1|66|2|129
1|69|3|198
2|64|1|64
2|67|2|131
2|70|3|201
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
