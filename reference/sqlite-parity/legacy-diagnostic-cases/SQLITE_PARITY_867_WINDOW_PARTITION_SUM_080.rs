// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_867_WINDOW_PARTITION_SUM_080

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 867,
        folder: r"SQLITE_PARITY_867_WINDOW_PARTITION_SUM_080",
        name: r"WINDOW_PARTITION_SUM_080",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_080.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 81), (2, 82), (0, 83), (1, 84), (2, 85), (0, 86), (1, 87), (2, 88), (0, 89);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|83|1|83
0|86|2|169
0|89|3|258
1|81|1|81
1|84|2|165
1|87|3|252
2|82|1|82
2|85|2|167
2|88|3|255
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
