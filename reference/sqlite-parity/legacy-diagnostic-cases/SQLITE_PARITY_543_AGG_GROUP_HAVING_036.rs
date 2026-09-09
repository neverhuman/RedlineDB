// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_543_AGG_GROUP_HAVING_036

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 543,
        folder: r"SQLITE_PARITY_543_AGG_GROUP_HAVING_036",
        name: r"AGG_GROUP_HAVING_036",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_036.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 37, 'g1'), (2, 38, 'g2'), (3, 39, 'g0'), (4, 40, 'g1'), (0, 41, 'g2'), (1, 42, 'g0'), (2, 43, 'g1'), (3, 44, 'g2'), (4, 45, 'g0'), (0, 46, 'g1'), (1, 47, 'g2'), (2, 48, 'g0'), (3, 49, 'g1'), (4, 50, 'g2'), (0, 51, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|39|51|45.0
g1|5|10|37|49|43.0
g2|5|10|38|50|44.0
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
