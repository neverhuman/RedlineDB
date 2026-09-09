// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_593_AGG_GROUP_HAVING_086

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 593,
        folder: r"SQLITE_PARITY_593_AGG_GROUP_HAVING_086",
        name: r"AGG_GROUP_HAVING_086",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_086.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 87, 'g1'), (2, 88, 'g2'), (3, 89, 'g0'), (4, 90, 'g1'), (0, 91, 'g2'), (1, 92, 'g0'), (2, 93, 'g1'), (3, 94, 'g2'), (4, 95, 'g0'), (0, 96, 'g1'), (1, 97, 'g2'), (2, 98, 'g0'), (3, 99, 'g1'), (4, 100, 'g2'), (0, 101, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|89|101|95.0
g1|5|10|87|99|93.0
g2|5|10|88|100|94.0
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
