// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_512_AGG_GROUP_HAVING_005

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 512,
        folder: r"SQLITE_PARITY_512_AGG_GROUP_HAVING_005",
        name: r"AGG_GROUP_HAVING_005",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_005.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 6, 'g1'), (2, 7, 'g2'), (3, 8, 'g0'), (4, 9, 'g1'), (0, 10, 'g2'), (1, 11, 'g0'), (2, 12, 'g1'), (3, 13, 'g2'), (4, 14, 'g0'), (0, 15, 'g1'), (1, 16, 'g2'), (2, 17, 'g0'), (3, 18, 'g1'), (4, 19, 'g2'), (0, 20, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|8|20|14.0
g1|5|10|6|18|12.0
g2|5|10|7|19|13.0
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
