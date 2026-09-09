// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_601_AGG_GROUP_HAVING_094

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 601,
        folder: r"SQLITE_PARITY_601_AGG_GROUP_HAVING_094",
        name: r"AGG_GROUP_HAVING_094",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_094.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 95, 'g1'), (2, 96, 'g2'), (3, 97, 'g0'), (4, 98, 'g1'), (0, 99, 'g2'), (1, 100, 'g0'), (2, 101, 'g1'), (3, 102, 'g2'), (4, 103, 'g0'), (0, 104, 'g1'), (1, 105, 'g2'), (2, 106, 'g0'), (3, 107, 'g1'), (4, 108, 'g2'), (0, 109, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|97|109|103.0
g1|5|10|95|107|101.0
g2|5|10|96|108|102.0
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
