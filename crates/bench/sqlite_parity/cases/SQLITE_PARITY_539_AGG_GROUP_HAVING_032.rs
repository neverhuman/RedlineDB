// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_539_AGG_GROUP_HAVING_032

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 539,
        folder: r"SQLITE_PARITY_539_AGG_GROUP_HAVING_032",
        name: r"AGG_GROUP_HAVING_032",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_032.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 33, 'g1'), (2, 34, 'g2'), (3, 35, 'g0'), (4, 36, 'g1'), (0, 37, 'g2'), (1, 38, 'g0'), (2, 39, 'g1'), (3, 40, 'g2'), (4, 41, 'g0'), (0, 42, 'g1'), (1, 43, 'g2'), (2, 44, 'g0'), (3, 45, 'g1'), (4, 46, 'g2'), (0, 47, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|35|47|41.0
g1|5|10|33|45|39.0
g2|5|10|34|46|40.0
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
