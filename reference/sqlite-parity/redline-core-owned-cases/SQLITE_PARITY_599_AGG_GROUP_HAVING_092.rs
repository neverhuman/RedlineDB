// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_599_AGG_GROUP_HAVING_092

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 599,
        folder: r"SQLITE_PARITY_599_AGG_GROUP_HAVING_092",
        name: r"AGG_GROUP_HAVING_092",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_092.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 93, 'g1'), (2, 94, 'g2'), (3, 95, 'g0'), (4, 96, 'g1'), (0, 97, 'g2'), (1, 98, 'g0'), (2, 99, 'g1'), (3, 100, 'g2'), (4, 101, 'g0'), (0, 102, 'g1'), (1, 103, 'g2'), (2, 104, 'g0'), (3, 105, 'g1'), (4, 106, 'g2'), (0, 107, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|95|107|101.0
g1|5|10|93|105|99.0
g2|5|10|94|106|100.0
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
