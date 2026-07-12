// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_856_WINDOW_PARTITION_SUM_069

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 856,
        folder: r"SQLITE_PARITY_856_WINDOW_PARTITION_SUM_069",
        name: r"WINDOW_PARTITION_SUM_069",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_069.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 70), (2, 71), (0, 72), (1, 73), (2, 74), (0, 75), (1, 76), (2, 77), (0, 78);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|72|1|72
0|75|2|147
0|78|3|225
1|70|1|70
1|73|2|143
1|76|3|219
2|71|1|71
2|74|2|145
2|77|3|222
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
