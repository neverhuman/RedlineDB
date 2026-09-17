// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_838_WINDOW_PARTITION_SUM_051

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 838,
        folder: r"SQLITE_PARITY_838_WINDOW_PARTITION_SUM_051",
        name: r"WINDOW_PARTITION_SUM_051",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_051.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 52), (2, 53), (0, 54), (1, 55), (2, 56), (0, 57), (1, 58), (2, 59), (0, 60);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|54|1|54
0|57|2|111
0|60|3|171
1|52|1|52
1|55|2|107
1|58|3|165
2|53|1|53
2|56|2|109
2|59|3|168
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
