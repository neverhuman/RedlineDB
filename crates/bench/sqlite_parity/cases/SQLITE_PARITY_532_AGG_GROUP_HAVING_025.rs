// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_532_AGG_GROUP_HAVING_025

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 532,
        folder: r"SQLITE_PARITY_532_AGG_GROUP_HAVING_025",
        name: r"AGG_GROUP_HAVING_025",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_025.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 26, 'g1'), (2, 27, 'g2'), (3, 28, 'g0'), (4, 29, 'g1'), (0, 30, 'g2'), (1, 31, 'g0'), (2, 32, 'g1'), (3, 33, 'g2'), (4, 34, 'g0'), (0, 35, 'g1'), (1, 36, 'g2'), (2, 37, 'g0'), (3, 38, 'g1'), (4, 39, 'g2'), (0, 40, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|28|40|34.0
g1|5|10|26|38|32.0
g2|5|10|27|39|33.0
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
