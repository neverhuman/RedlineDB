// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_523_AGG_GROUP_HAVING_016

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 523,
        folder: r"SQLITE_PARITY_523_AGG_GROUP_HAVING_016",
        name: r"AGG_GROUP_HAVING_016",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_016.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 17, 'g1'), (2, 18, 'g2'), (3, 19, 'g0'), (4, 20, 'g1'), (0, 21, 'g2'), (1, 22, 'g0'), (2, 23, 'g1'), (3, 24, 'g2'), (4, 25, 'g0'), (0, 26, 'g1'), (1, 27, 'g2'), (2, 28, 'g0'), (3, 29, 'g1'), (4, 30, 'g2'), (0, 31, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|19|31|25.0
g1|5|10|17|29|23.0
g2|5|10|18|30|24.0
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
