// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_581_AGG_GROUP_HAVING_074

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 581,
        folder: r"SQLITE_PARITY_581_AGG_GROUP_HAVING_074",
        name: r"AGG_GROUP_HAVING_074",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_074.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 75, 'g1'), (2, 76, 'g2'), (3, 77, 'g0'), (4, 78, 'g1'), (0, 79, 'g2'), (1, 80, 'g0'), (2, 81, 'g1'), (3, 82, 'g2'), (4, 83, 'g0'), (0, 84, 'g1'), (1, 85, 'g2'), (2, 86, 'g0'), (3, 87, 'g1'), (4, 88, 'g2'), (0, 89, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|77|89|83.0
g1|5|10|75|87|81.0
g2|5|10|76|88|82.0
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
