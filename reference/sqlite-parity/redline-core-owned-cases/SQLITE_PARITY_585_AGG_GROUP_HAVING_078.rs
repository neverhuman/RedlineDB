// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_585_AGG_GROUP_HAVING_078

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 585,
        folder: r"SQLITE_PARITY_585_AGG_GROUP_HAVING_078",
        name: r"AGG_GROUP_HAVING_078",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_078.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 79, 'g1'), (2, 80, 'g2'), (3, 81, 'g0'), (4, 82, 'g1'), (0, 83, 'g2'), (1, 84, 'g0'), (2, 85, 'g1'), (3, 86, 'g2'), (4, 87, 'g0'), (0, 88, 'g1'), (1, 89, 'g2'), (2, 90, 'g0'), (3, 91, 'g1'), (4, 92, 'g2'), (0, 93, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|81|93|87.0
g1|5|10|79|91|85.0
g2|5|10|80|92|86.0
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
