// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_579_AGG_GROUP_HAVING_072

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 579,
        folder: r"SQLITE_PARITY_579_AGG_GROUP_HAVING_072",
        name: r"AGG_GROUP_HAVING_072",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_072.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 73, 'g1'), (2, 74, 'g2'), (3, 75, 'g0'), (4, 76, 'g1'), (0, 77, 'g2'), (1, 78, 'g0'), (2, 79, 'g1'), (3, 80, 'g2'), (4, 81, 'g0'), (0, 82, 'g1'), (1, 83, 'g2'), (2, 84, 'g0'), (3, 85, 'g1'), (4, 86, 'g2'), (0, 87, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|75|87|81.0
g1|5|10|73|85|79.0
g2|5|10|74|86|80.0
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
