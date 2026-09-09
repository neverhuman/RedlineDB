// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_857_WINDOW_PARTITION_SUM_070

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 857,
        folder: r"SQLITE_PARITY_857_WINDOW_PARTITION_SUM_070",
        name: r"WINDOW_PARTITION_SUM_070",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_070.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 71), (2, 72), (0, 73), (1, 74), (2, 75), (0, 76), (1, 77), (2, 78), (0, 79);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|73|1|73
0|76|2|149
0|79|3|228
1|71|1|71
1|74|2|145
1|77|3|222
2|72|1|72
2|75|2|147
2|78|3|225
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
