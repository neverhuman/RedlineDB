// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_531_AGG_GROUP_HAVING_024

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 531,
        folder: r"SQLITE_PARITY_531_AGG_GROUP_HAVING_024",
        name: r"AGG_GROUP_HAVING_024",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_024.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 25, 'g1'), (2, 26, 'g2'), (3, 27, 'g0'), (4, 28, 'g1'), (0, 29, 'g2'), (1, 30, 'g0'), (2, 31, 'g1'), (3, 32, 'g2'), (4, 33, 'g0'), (0, 34, 'g1'), (1, 35, 'g2'), (2, 36, 'g0'), (3, 37, 'g1'), (4, 38, 'g2'), (0, 39, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|27|39|33.0
g1|5|10|25|37|31.0
g2|5|10|26|38|32.0
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
