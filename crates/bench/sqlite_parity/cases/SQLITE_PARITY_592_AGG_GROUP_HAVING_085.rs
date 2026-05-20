// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_592_AGG_GROUP_HAVING_085

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 592,
        folder: r"SQLITE_PARITY_592_AGG_GROUP_HAVING_085",
        name: r"AGG_GROUP_HAVING_085",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_085.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 86, 'g1'), (2, 87, 'g2'), (3, 88, 'g0'), (4, 89, 'g1'), (0, 90, 'g2'), (1, 91, 'g0'), (2, 92, 'g1'), (3, 93, 'g2'), (4, 94, 'g0'), (0, 95, 'g1'), (1, 96, 'g2'), (2, 97, 'g0'), (3, 98, 'g1'), (4, 99, 'g2'), (0, 100, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|88|100|94.0
g1|5|10|86|98|92.0
g2|5|10|87|99|93.0
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
