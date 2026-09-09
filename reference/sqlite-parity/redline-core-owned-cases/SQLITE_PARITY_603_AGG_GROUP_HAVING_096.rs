// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_603_AGG_GROUP_HAVING_096

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 603,
        folder: r"SQLITE_PARITY_603_AGG_GROUP_HAVING_096",
        name: r"AGG_GROUP_HAVING_096",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_096.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 97, 'g1'), (2, 98, 'g2'), (3, 99, 'g0'), (4, 100, 'g1'), (0, 101, 'g2'), (1, 102, 'g0'), (2, 103, 'g1'), (3, 104, 'g2'), (4, 105, 'g0'), (0, 106, 'g1'), (1, 107, 'g2'), (2, 108, 'g0'), (3, 109, 'g1'), (4, 110, 'g2'), (0, 111, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|99|111|105.0
g1|5|10|97|109|103.0
g2|5|10|98|110|104.0
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
