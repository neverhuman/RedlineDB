// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_519_AGG_GROUP_HAVING_012

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 519,
        folder: r"SQLITE_PARITY_519_AGG_GROUP_HAVING_012",
        name: r"AGG_GROUP_HAVING_012",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_012.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 13, 'g1'), (2, 14, 'g2'), (3, 15, 'g0'), (4, 16, 'g1'), (0, 17, 'g2'), (1, 18, 'g0'), (2, 19, 'g1'), (3, 20, 'g2'), (4, 21, 'g0'), (0, 22, 'g1'), (1, 23, 'g2'), (2, 24, 'g0'), (3, 25, 'g1'), (4, 26, 'g2'), (0, 27, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|15|27|21.0
g1|5|10|13|25|19.0
g2|5|10|14|26|20.0
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
