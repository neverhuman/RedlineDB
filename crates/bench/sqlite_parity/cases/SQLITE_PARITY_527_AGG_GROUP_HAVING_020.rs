// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_527_AGG_GROUP_HAVING_020

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 527,
        folder: r"SQLITE_PARITY_527_AGG_GROUP_HAVING_020",
        name: r"AGG_GROUP_HAVING_020",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_020.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 21, 'g1'), (2, 22, 'g2'), (3, 23, 'g0'), (4, 24, 'g1'), (0, 25, 'g2'), (1, 26, 'g0'), (2, 27, 'g1'), (3, 28, 'g2'), (4, 29, 'g0'), (0, 30, 'g1'), (1, 31, 'g2'), (2, 32, 'g0'), (3, 33, 'g1'), (4, 34, 'g2'), (0, 35, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|23|35|29.0
g1|5|10|21|33|27.0
g2|5|10|22|34|28.0
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
