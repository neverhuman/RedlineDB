// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_580_AGG_GROUP_HAVING_073

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 580,
        folder: r"SQLITE_PARITY_580_AGG_GROUP_HAVING_073",
        name: r"AGG_GROUP_HAVING_073",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_073.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 74, 'g1'), (2, 75, 'g2'), (3, 76, 'g0'), (4, 77, 'g1'), (0, 78, 'g2'), (1, 79, 'g0'), (2, 80, 'g1'), (3, 81, 'g2'), (4, 82, 'g0'), (0, 83, 'g1'), (1, 84, 'g2'), (2, 85, 'g0'), (3, 86, 'g1'), (4, 87, 'g2'), (0, 88, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|76|88|82.0
g1|5|10|74|86|80.0
g2|5|10|75|87|81.0
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
