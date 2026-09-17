// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_862_WINDOW_PARTITION_SUM_075

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 862,
        folder: r"SQLITE_PARITY_862_WINDOW_PARTITION_SUM_075",
        name: r"WINDOW_PARTITION_SUM_075",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_075.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 76), (2, 77), (0, 78), (1, 79), (2, 80), (0, 81), (1, 82), (2, 83), (0, 84);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|78|1|78
0|81|2|159
0|84|3|243
1|76|1|76
1|79|2|155
1|82|3|237
2|77|1|77
2|80|2|157
2|83|3|240
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
