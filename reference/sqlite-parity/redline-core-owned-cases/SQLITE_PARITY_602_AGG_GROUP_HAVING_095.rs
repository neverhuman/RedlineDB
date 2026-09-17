// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_602_AGG_GROUP_HAVING_095

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 602,
        folder: r"SQLITE_PARITY_602_AGG_GROUP_HAVING_095",
        name: r"AGG_GROUP_HAVING_095",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_095.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 96, 'g1'), (2, 97, 'g2'), (3, 98, 'g0'), (4, 99, 'g1'), (0, 100, 'g2'), (1, 101, 'g0'), (2, 102, 'g1'), (3, 103, 'g2'), (4, 104, 'g0'), (0, 105, 'g1'), (1, 106, 'g2'), (2, 107, 'g0'), (3, 108, 'g1'), (4, 109, 'g2'), (0, 110, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|98|110|104.0
g1|5|10|96|108|102.0
g2|5|10|97|109|103.0
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
