// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_836_WINDOW_PARTITION_SUM_049

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 836,
        folder: r"SQLITE_PARITY_836_WINDOW_PARTITION_SUM_049",
        name: r"WINDOW_PARTITION_SUM_049",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_049.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 50), (2, 51), (0, 52), (1, 53), (2, 54), (0, 55), (1, 56), (2, 57), (0, 58);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|52|1|52
0|55|2|107
0|58|3|165
1|50|1|50
1|53|2|103
1|56|3|159
2|51|1|51
2|54|2|105
2|57|3|162
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
