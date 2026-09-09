// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_567_AGG_GROUP_HAVING_060

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 567,
        folder: r"SQLITE_PARITY_567_AGG_GROUP_HAVING_060",
        name: r"AGG_GROUP_HAVING_060",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_060.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 61, 'g1'), (2, 62, 'g2'), (3, 63, 'g0'), (4, 64, 'g1'), (0, 65, 'g2'), (1, 66, 'g0'), (2, 67, 'g1'), (3, 68, 'g2'), (4, 69, 'g0'), (0, 70, 'g1'), (1, 71, 'g2'), (2, 72, 'g0'), (3, 73, 'g1'), (4, 74, 'g2'), (0, 75, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|63|75|69.0
g1|5|10|61|73|67.0
g2|5|10|62|74|68.0
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
