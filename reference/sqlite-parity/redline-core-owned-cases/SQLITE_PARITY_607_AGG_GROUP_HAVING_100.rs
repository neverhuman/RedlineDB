// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_607_AGG_GROUP_HAVING_100

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 607,
        folder: r"SQLITE_PARITY_607_AGG_GROUP_HAVING_100",
        name: r"AGG_GROUP_HAVING_100",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_100.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 101, 'g1'), (2, 102, 'g2'), (3, 103, 'g0'), (4, 104, 'g1'), (0, 105, 'g2'), (1, 106, 'g0'), (2, 107, 'g1'), (3, 108, 'g2'), (4, 109, 'g0'), (0, 110, 'g1'), (1, 111, 'g2'), (2, 112, 'g0'), (3, 113, 'g1'), (4, 114, 'g2'), (0, 115, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|103|115|109.0
g1|5|10|101|113|107.0
g2|5|10|102|114|108.0
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
