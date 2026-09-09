// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_573_AGG_GROUP_HAVING_066

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 573,
        folder: r"SQLITE_PARITY_573_AGG_GROUP_HAVING_066",
        name: r"AGG_GROUP_HAVING_066",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_066.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 67, 'g1'), (2, 68, 'g2'), (3, 69, 'g0'), (4, 70, 'g1'), (0, 71, 'g2'), (1, 72, 'g0'), (2, 73, 'g1'), (3, 74, 'g2'), (4, 75, 'g0'), (0, 76, 'g1'), (1, 77, 'g2'), (2, 78, 'g0'), (3, 79, 'g1'), (4, 80, 'g2'), (0, 81, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|69|81|75.0
g1|5|10|67|79|73.0
g2|5|10|68|80|74.0
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
