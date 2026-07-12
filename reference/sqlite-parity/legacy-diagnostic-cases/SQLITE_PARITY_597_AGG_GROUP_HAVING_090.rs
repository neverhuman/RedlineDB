// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_597_AGG_GROUP_HAVING_090

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 597,
        folder: r"SQLITE_PARITY_597_AGG_GROUP_HAVING_090",
        name: r"AGG_GROUP_HAVING_090",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_090.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 91, 'g1'), (2, 92, 'g2'), (3, 93, 'g0'), (4, 94, 'g1'), (0, 95, 'g2'), (1, 96, 'g0'), (2, 97, 'g1'), (3, 98, 'g2'), (4, 99, 'g0'), (0, 100, 'g1'), (1, 101, 'g2'), (2, 102, 'g0'), (3, 103, 'g1'), (4, 104, 'g2'), (0, 105, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|93|105|99.0
g1|5|10|91|103|97.0
g2|5|10|92|104|98.0
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
