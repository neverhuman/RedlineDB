// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_547_AGG_GROUP_HAVING_040

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 547,
        folder: r"SQLITE_PARITY_547_AGG_GROUP_HAVING_040",
        name: r"AGG_GROUP_HAVING_040",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_040.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 41, 'g1'), (2, 42, 'g2'), (3, 43, 'g0'), (4, 44, 'g1'), (0, 45, 'g2'), (1, 46, 'g0'), (2, 47, 'g1'), (3, 48, 'g2'), (4, 49, 'g0'), (0, 50, 'g1'), (1, 51, 'g2'), (2, 52, 'g0'), (3, 53, 'g1'), (4, 54, 'g2'), (0, 55, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|43|55|49.0
g1|5|10|41|53|47.0
g2|5|10|42|54|48.0
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
