// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_816_WINDOW_PARTITION_SUM_029

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 816,
        folder: r"SQLITE_PARITY_816_WINDOW_PARTITION_SUM_029",
        name: r"WINDOW_PARTITION_SUM_029",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_029.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 30), (2, 31), (0, 32), (1, 33), (2, 34), (0, 35), (1, 36), (2, 37), (0, 38);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|32|1|32
0|35|2|67
0|38|3|105
1|30|1|30
1|33|2|63
1|36|3|99
2|31|1|31
2|34|2|65
2|37|3|102
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
