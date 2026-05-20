// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_600_AGG_GROUP_HAVING_093

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 600,
        folder: r"SQLITE_PARITY_600_AGG_GROUP_HAVING_093",
        name: r"AGG_GROUP_HAVING_093",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_093.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 94, 'g1'), (2, 95, 'g2'), (3, 96, 'g0'), (4, 97, 'g1'), (0, 98, 'g2'), (1, 99, 'g0'), (2, 100, 'g1'), (3, 101, 'g2'), (4, 102, 'g0'), (0, 103, 'g1'), (1, 104, 'g2'), (2, 105, 'g0'), (3, 106, 'g1'), (4, 107, 'g2'), (0, 108, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|96|108|102.0
g1|5|10|94|106|100.0
g2|5|10|95|107|101.0
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
