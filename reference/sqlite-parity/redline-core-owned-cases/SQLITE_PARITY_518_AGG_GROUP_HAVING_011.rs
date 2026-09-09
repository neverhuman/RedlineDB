// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_518_AGG_GROUP_HAVING_011

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 518,
        folder: r"SQLITE_PARITY_518_AGG_GROUP_HAVING_011",
        name: r"AGG_GROUP_HAVING_011",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_011.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 12, 'g1'), (2, 13, 'g2'), (3, 14, 'g0'), (4, 15, 'g1'), (0, 16, 'g2'), (1, 17, 'g0'), (2, 18, 'g1'), (3, 19, 'g2'), (4, 20, 'g0'), (0, 21, 'g1'), (1, 22, 'g2'), (2, 23, 'g0'), (3, 24, 'g1'), (4, 25, 'g2'), (0, 26, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|14|26|20.0
g1|5|10|12|24|18.0
g2|5|10|13|25|19.0
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
