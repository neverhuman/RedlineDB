// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_864_WINDOW_PARTITION_SUM_077

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 864,
        folder: r"SQLITE_PARITY_864_WINDOW_PARTITION_SUM_077",
        name: r"WINDOW_PARTITION_SUM_077",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_077.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 78), (2, 79), (0, 80), (1, 81), (2, 82), (0, 83), (1, 84), (2, 85), (0, 86);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|80|1|80
0|83|2|163
0|86|3|249
1|78|1|78
1|81|2|159
1|84|3|243
2|79|1|79
2|82|2|161
2|85|3|246
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
