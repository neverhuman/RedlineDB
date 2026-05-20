// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_835_WINDOW_PARTITION_SUM_048

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 835,
        folder: r"SQLITE_PARITY_835_WINDOW_PARTITION_SUM_048",
        name: r"WINDOW_PARTITION_SUM_048",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_048.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 49), (2, 50), (0, 51), (1, 52), (2, 53), (0, 54), (1, 55), (2, 56), (0, 57);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|51|1|51
0|54|2|105
0|57|3|162
1|49|1|49
1|52|2|101
1|55|3|156
2|50|1|50
2|53|2|103
2|56|3|159
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
