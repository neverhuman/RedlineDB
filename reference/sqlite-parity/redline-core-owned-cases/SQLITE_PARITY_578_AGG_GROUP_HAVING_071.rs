// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_578_AGG_GROUP_HAVING_071

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 578,
        folder: r"SQLITE_PARITY_578_AGG_GROUP_HAVING_071",
        name: r"AGG_GROUP_HAVING_071",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_071.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 72, 'g1'), (2, 73, 'g2'), (3, 74, 'g0'), (4, 75, 'g1'), (0, 76, 'g2'), (1, 77, 'g0'), (2, 78, 'g1'), (3, 79, 'g2'), (4, 80, 'g0'), (0, 81, 'g1'), (1, 82, 'g2'), (2, 83, 'g0'), (3, 84, 'g1'), (4, 85, 'g2'), (0, 86, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|74|86|80.0
g1|5|10|72|84|78.0
g2|5|10|73|85|79.0
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
