// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_848_WINDOW_PARTITION_SUM_061

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 848,
        folder: r"SQLITE_PARITY_848_WINDOW_PARTITION_SUM_061",
        name: r"WINDOW_PARTITION_SUM_061",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_061.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 62), (2, 63), (0, 64), (1, 65), (2, 66), (0, 67), (1, 68), (2, 69), (0, 70);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|64|1|64
0|67|2|131
0|70|3|201
1|62|1|62
1|65|2|127
1|68|3|195
2|63|1|63
2|66|2|129
2|69|3|198
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
