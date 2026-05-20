// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_587_AGG_GROUP_HAVING_080

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 587,
        folder: r"SQLITE_PARITY_587_AGG_GROUP_HAVING_080",
        name: r"AGG_GROUP_HAVING_080",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_080.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 81, 'g1'), (2, 82, 'g2'), (3, 83, 'g0'), (4, 84, 'g1'), (0, 85, 'g2'), (1, 86, 'g0'), (2, 87, 'g1'), (3, 88, 'g2'), (4, 89, 'g0'), (0, 90, 'g1'), (1, 91, 'g2'), (2, 92, 'g0'), (3, 93, 'g1'), (4, 94, 'g2'), (0, 95, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|83|95|89.0
g1|5|10|81|93|87.0
g2|5|10|82|94|88.0
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
