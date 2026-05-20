// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_554_AGG_GROUP_HAVING_047

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 554,
        folder: r"SQLITE_PARITY_554_AGG_GROUP_HAVING_047",
        name: r"AGG_GROUP_HAVING_047",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_047.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 48, 'g1'), (2, 49, 'g2'), (3, 50, 'g0'), (4, 51, 'g1'), (0, 52, 'g2'), (1, 53, 'g0'), (2, 54, 'g1'), (3, 55, 'g2'), (4, 56, 'g0'), (0, 57, 'g1'), (1, 58, 'g2'), (2, 59, 'g0'), (3, 60, 'g1'), (4, 61, 'g2'), (0, 62, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|50|62|56.0
g1|5|10|48|60|54.0
g2|5|10|49|61|55.0
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
