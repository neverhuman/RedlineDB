// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_818_WINDOW_PARTITION_SUM_031

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 818,
        folder: r"SQLITE_PARITY_818_WINDOW_PARTITION_SUM_031",
        name: r"WINDOW_PARTITION_SUM_031",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_031.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 32), (2, 33), (0, 34), (1, 35), (2, 36), (0, 37), (1, 38), (2, 39), (0, 40);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|34|1|34
0|37|2|71
0|40|3|111
1|32|1|32
1|35|2|67
1|38|3|105
2|33|1|33
2|36|2|69
2|39|3|108
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
