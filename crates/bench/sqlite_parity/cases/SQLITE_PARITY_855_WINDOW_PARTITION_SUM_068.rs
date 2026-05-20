// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_855_WINDOW_PARTITION_SUM_068

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 855,
        folder: r"SQLITE_PARITY_855_WINDOW_PARTITION_SUM_068",
        name: r"WINDOW_PARTITION_SUM_068",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_068.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 69), (2, 70), (0, 71), (1, 72), (2, 73), (0, 74), (1, 75), (2, 76), (0, 77);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|71|1|71
0|74|2|145
0|77|3|222
1|69|1|69
1|72|2|141
1|75|3|216
2|70|1|70
2|73|2|143
2|76|3|219
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
