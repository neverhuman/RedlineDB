// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_595_AGG_GROUP_HAVING_088

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 595,
        folder: r"SQLITE_PARITY_595_AGG_GROUP_HAVING_088",
        name: r"AGG_GROUP_HAVING_088",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_088.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 89, 'g1'), (2, 90, 'g2'), (3, 91, 'g0'), (4, 92, 'g1'), (0, 93, 'g2'), (1, 94, 'g0'), (2, 95, 'g1'), (3, 96, 'g2'), (4, 97, 'g0'), (0, 98, 'g1'), (1, 99, 'g2'), (2, 100, 'g0'), (3, 101, 'g1'), (4, 102, 'g2'), (0, 103, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|91|103|97.0
g1|5|10|89|101|95.0
g2|5|10|90|102|96.0
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
