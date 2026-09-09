// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_598_AGG_GROUP_HAVING_091

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 598,
        folder: r"SQLITE_PARITY_598_AGG_GROUP_HAVING_091",
        name: r"AGG_GROUP_HAVING_091",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_091.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 92, 'g1'), (2, 93, 'g2'), (3, 94, 'g0'), (4, 95, 'g1'), (0, 96, 'g2'), (1, 97, 'g0'), (2, 98, 'g1'), (3, 99, 'g2'), (4, 100, 'g0'), (0, 101, 'g1'), (1, 102, 'g2'), (2, 103, 'g0'), (3, 104, 'g1'), (4, 105, 'g2'), (0, 106, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|94|106|100.0
g1|5|10|92|104|98.0
g2|5|10|93|105|99.0
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
