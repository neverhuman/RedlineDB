// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_821_WINDOW_PARTITION_SUM_034

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 821,
        folder: r"SQLITE_PARITY_821_WINDOW_PARTITION_SUM_034",
        name: r"WINDOW_PARTITION_SUM_034",
        category: r"GEN_SQL_WINDOW",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_034.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(part INT, val INT);
INSERT INTO t VALUES (1, 35), (2, 36), (0, 37), (1, 38), (2, 39), (0, 40), (1, 41), (2, 42), (0, 43);
SELECT part, val, row_number() OVER (PARTITION BY part ORDER BY val), sum(val) OVER (PARTITION BY part ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
FROM t ORDER BY part, val;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0|37|1|37
0|40|2|77
0|43|3|120
1|35|1|35
1|38|2|73
1|41|3|114
2|36|1|36
2|39|2|75
2|42|3|117
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
