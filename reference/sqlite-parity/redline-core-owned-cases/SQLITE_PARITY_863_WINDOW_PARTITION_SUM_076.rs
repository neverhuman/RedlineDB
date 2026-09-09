// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_863_WINDOW_PARTITION_SUM_076

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 863,
        folder: r"SQLITE_PARITY_863_WINDOW_PARTITION_SUM_076",
        name: r"WINDOW_PARTITION_SUM_076",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_076.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 77), (2, 78), (0, 79), (1, 80), (2, 81), (0, 82), (1, 83), (2, 84), (0, 85);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|79|1|79
0|82|2|161
0|85|3|246
1|77|1|77
1|80|2|157
1|83|3|240
2|78|1|78
2|81|2|159
2|84|3|243
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
