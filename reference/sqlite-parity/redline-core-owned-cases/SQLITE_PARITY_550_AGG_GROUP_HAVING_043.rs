// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_550_AGG_GROUP_HAVING_043

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 550,
        folder: r"SQLITE_PARITY_550_AGG_GROUP_HAVING_043",
        name: r"AGG_GROUP_HAVING_043",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_043.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 44, 'g1'), (2, 45, 'g2'), (3, 46, 'g0'), (4, 47, 'g1'), (0, 48, 'g2'), (1, 49, 'g0'), (2, 50, 'g1'), (3, 51, 'g2'), (4, 52, 'g0'), (0, 53, 'g1'), (1, 54, 'g2'), (2, 55, 'g0'), (3, 56, 'g1'), (4, 57, 'g2'), (0, 58, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|46|58|52.0
g1|5|10|44|56|50.0
g2|5|10|45|57|51.0
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
