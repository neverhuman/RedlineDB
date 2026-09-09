// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_586_AGG_GROUP_HAVING_079

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 586,
        folder: r"SQLITE_PARITY_586_AGG_GROUP_HAVING_079",
        name: r"AGG_GROUP_HAVING_079",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_079.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 80, 'g1'), (2, 81, 'g2'), (3, 82, 'g0'), (4, 83, 'g1'), (0, 84, 'g2'), (1, 85, 'g0'), (2, 86, 'g1'), (3, 87, 'g2'), (4, 88, 'g0'), (0, 89, 'g1'), (1, 90, 'g2'), (2, 91, 'g0'), (3, 92, 'g1'), (4, 93, 'g2'), (0, 94, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|82|94|88.0
g1|5|10|80|92|86.0
g2|5|10|81|93|87.0
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
