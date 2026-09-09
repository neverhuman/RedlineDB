// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_555_AGG_GROUP_HAVING_048

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 555,
        folder: r"SQLITE_PARITY_555_AGG_GROUP_HAVING_048",
        name: r"AGG_GROUP_HAVING_048",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_048.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 49, 'g1'), (2, 50, 'g2'), (3, 51, 'g0'), (4, 52, 'g1'), (0, 53, 'g2'), (1, 54, 'g0'), (2, 55, 'g1'), (3, 56, 'g2'), (4, 57, 'g0'), (0, 58, 'g1'), (1, 59, 'g2'), (2, 60, 'g0'), (3, 61, 'g1'), (4, 62, 'g2'), (0, 63, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|51|63|57.0
g1|5|10|49|61|55.0
g2|5|10|50|62|56.0
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
