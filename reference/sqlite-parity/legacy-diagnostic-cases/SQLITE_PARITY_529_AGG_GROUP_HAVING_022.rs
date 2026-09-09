// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_529_AGG_GROUP_HAVING_022

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 529,
        folder: r"SQLITE_PARITY_529_AGG_GROUP_HAVING_022",
        name: r"AGG_GROUP_HAVING_022",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_022.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 23, 'g1'), (2, 24, 'g2'), (3, 25, 'g0'), (4, 26, 'g1'), (0, 27, 'g2'), (1, 28, 'g0'), (2, 29, 'g1'), (3, 30, 'g2'), (4, 31, 'g0'), (0, 32, 'g1'), (1, 33, 'g2'), (2, 34, 'g0'), (3, 35, 'g1'), (4, 36, 'g2'), (0, 37, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|25|37|31.0
g1|5|10|23|35|29.0
g2|5|10|24|36|30.0
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
