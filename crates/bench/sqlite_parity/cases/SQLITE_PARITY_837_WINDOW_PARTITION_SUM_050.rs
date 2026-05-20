// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_837_WINDOW_PARTITION_SUM_050

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 837,
        folder: r"SQLITE_PARITY_837_WINDOW_PARTITION_SUM_050",
        name: r"WINDOW_PARTITION_SUM_050",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_050.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 51), (2, 52), (0, 53), (1, 54), (2, 55), (0, 56), (1, 57), (2, 58), (0, 59);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|53|1|53
0|56|2|109
0|59|3|168
1|51|1|51
1|54|2|105
1|57|3|162
2|52|1|52
2|55|2|107
2|58|3|165
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
