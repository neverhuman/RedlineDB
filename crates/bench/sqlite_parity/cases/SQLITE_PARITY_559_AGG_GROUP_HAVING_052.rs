// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_559_AGG_GROUP_HAVING_052

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 559,
        folder: r"SQLITE_PARITY_559_AGG_GROUP_HAVING_052",
        name: r"AGG_GROUP_HAVING_052",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_052.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 53, 'g1'), (2, 54, 'g2'), (3, 55, 'g0'), (4, 56, 'g1'), (0, 57, 'g2'), (1, 58, 'g0'), (2, 59, 'g1'), (3, 60, 'g2'), (4, 61, 'g0'), (0, 62, 'g1'), (1, 63, 'g2'), (2, 64, 'g0'), (3, 65, 'g1'), (4, 66, 'g2'), (0, 67, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|55|67|61.0
g1|5|10|53|65|59.0
g2|5|10|54|66|60.0
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
