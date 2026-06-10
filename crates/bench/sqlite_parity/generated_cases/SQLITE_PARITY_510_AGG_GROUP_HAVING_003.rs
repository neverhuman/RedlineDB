// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_510_AGG_GROUP_HAVING_003

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 510,
        folder: r"SQLITE_PARITY_510_AGG_GROUP_HAVING_003",
        name: r"AGG_GROUP_HAVING_003",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_003.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 4, 'g1'), (2, 5, 'g2'), (3, 6, 'g0'), (4, 7, 'g1'), (0, 8, 'g2'), (1, 9, 'g0'), (2, 10, 'g1'), (3, 11, 'g2'), (4, 12, 'g0'), (0, 13, 'g1'), (1, 14, 'g2'), (2, 15, 'g0'), (3, 16, 'g1'), (4, 17, 'g2'), (0, 18, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|6|18|12.0
g1|5|10|4|16|10.0
g2|5|10|5|17|11.0
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
