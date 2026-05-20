// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_511_AGG_GROUP_HAVING_004

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 511,
        folder: r"SQLITE_PARITY_511_AGG_GROUP_HAVING_004",
        name: r"AGG_GROUP_HAVING_004",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_004.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 5, 'g1'), (2, 6, 'g2'), (3, 7, 'g0'), (4, 8, 'g1'), (0, 9, 'g2'), (1, 10, 'g0'), (2, 11, 'g1'), (3, 12, 'g2'), (4, 13, 'g0'), (0, 14, 'g1'), (1, 15, 'g2'), (2, 16, 'g0'), (3, 17, 'g1'), (4, 18, 'g2'), (0, 19, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|7|19|13.0
g1|5|10|5|17|11.0
g2|5|10|6|18|12.0
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
