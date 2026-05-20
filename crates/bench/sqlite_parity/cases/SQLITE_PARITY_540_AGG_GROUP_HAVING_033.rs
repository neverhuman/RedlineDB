// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_540_AGG_GROUP_HAVING_033

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 540,
        folder: r"SQLITE_PARITY_540_AGG_GROUP_HAVING_033",
        name: r"AGG_GROUP_HAVING_033",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_033.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 34, 'g1'), (2, 35, 'g2'), (3, 36, 'g0'), (4, 37, 'g1'), (0, 38, 'g2'), (1, 39, 'g0'), (2, 40, 'g1'), (3, 41, 'g2'), (4, 42, 'g0'), (0, 43, 'g1'), (1, 44, 'g2'), (2, 45, 'g0'), (3, 46, 'g1'), (4, 47, 'g2'), (0, 48, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|36|48|42.0
g1|5|10|34|46|40.0
g2|5|10|35|47|41.0
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
