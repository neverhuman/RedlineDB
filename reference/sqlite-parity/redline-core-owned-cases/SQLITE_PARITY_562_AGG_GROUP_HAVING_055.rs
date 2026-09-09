// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_562_AGG_GROUP_HAVING_055

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 562,
        folder: r"SQLITE_PARITY_562_AGG_GROUP_HAVING_055",
        name: r"AGG_GROUP_HAVING_055",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_055.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 56, 'g1'), (2, 57, 'g2'), (3, 58, 'g0'), (4, 59, 'g1'), (0, 60, 'g2'), (1, 61, 'g0'), (2, 62, 'g1'), (3, 63, 'g2'), (4, 64, 'g0'), (0, 65, 'g1'), (1, 66, 'g2'), (2, 67, 'g0'), (3, 68, 'g1'), (4, 69, 'g2'), (0, 70, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|58|70|64.0
g1|5|10|56|68|62.0
g2|5|10|57|69|63.0
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
