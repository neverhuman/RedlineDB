// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_514_AGG_GROUP_HAVING_007

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 514,
        folder: r"SQLITE_PARITY_514_AGG_GROUP_HAVING_007",
        name: r"AGG_GROUP_HAVING_007",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_007.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 8, 'g1'), (2, 9, 'g2'), (3, 10, 'g0'), (4, 11, 'g1'), (0, 12, 'g2'), (1, 13, 'g0'), (2, 14, 'g1'), (3, 15, 'g2'), (4, 16, 'g0'), (0, 17, 'g1'), (1, 18, 'g2'), (2, 19, 'g0'), (3, 20, 'g1'), (4, 21, 'g2'), (0, 22, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|10|22|16.0
g1|5|10|8|20|14.0
g2|5|10|9|21|15.0
5|3
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
