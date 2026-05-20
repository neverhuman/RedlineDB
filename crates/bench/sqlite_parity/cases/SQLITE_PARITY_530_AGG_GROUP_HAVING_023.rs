// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_530_AGG_GROUP_HAVING_023

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 530,
        folder: r"SQLITE_PARITY_530_AGG_GROUP_HAVING_023",
        name: r"AGG_GROUP_HAVING_023",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_023.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 24, 'g1'), (2, 25, 'g2'), (3, 26, 'g0'), (4, 27, 'g1'), (0, 28, 'g2'), (1, 29, 'g0'), (2, 30, 'g1'), (3, 31, 'g2'), (4, 32, 'g0'), (0, 33, 'g1'), (1, 34, 'g2'), (2, 35, 'g0'), (3, 36, 'g1'), (4, 37, 'g2'), (0, 38, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|26|38|32.0
g1|5|10|24|36|30.0
g2|5|10|25|37|31.0
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
