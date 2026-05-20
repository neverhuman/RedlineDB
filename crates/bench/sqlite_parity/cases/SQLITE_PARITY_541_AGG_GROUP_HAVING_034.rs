// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_541_AGG_GROUP_HAVING_034

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 541,
        folder: r"SQLITE_PARITY_541_AGG_GROUP_HAVING_034",
        name: r"AGG_GROUP_HAVING_034",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_034.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 35, 'g1'), (2, 36, 'g2'), (3, 37, 'g0'), (4, 38, 'g1'), (0, 39, 'g2'), (1, 40, 'g0'), (2, 41, 'g1'), (3, 42, 'g2'), (4, 43, 'g0'), (0, 44, 'g1'), (1, 45, 'g2'), (2, 46, 'g0'), (3, 47, 'g1'), (4, 48, 'g2'), (0, 49, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|37|49|43.0
g1|5|10|35|47|41.0
g2|5|10|36|48|42.0
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
