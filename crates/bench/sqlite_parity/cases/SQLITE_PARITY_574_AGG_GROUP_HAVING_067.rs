// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_574_AGG_GROUP_HAVING_067

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 574,
        folder: r"SQLITE_PARITY_574_AGG_GROUP_HAVING_067",
        name: r"AGG_GROUP_HAVING_067",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_067.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 68, 'g1'), (2, 69, 'g2'), (3, 70, 'g0'), (4, 71, 'g1'), (0, 72, 'g2'), (1, 73, 'g0'), (2, 74, 'g1'), (3, 75, 'g2'), (4, 76, 'g0'), (0, 77, 'g1'), (1, 78, 'g2'), (2, 79, 'g0'), (3, 80, 'g1'), (4, 81, 'g2'), (0, 82, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|70|82|76.0
g1|5|10|68|80|74.0
g2|5|10|69|81|75.0
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
