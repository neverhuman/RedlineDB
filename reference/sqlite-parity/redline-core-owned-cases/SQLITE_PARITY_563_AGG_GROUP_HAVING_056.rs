// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_563_AGG_GROUP_HAVING_056

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 563,
        folder: r"SQLITE_PARITY_563_AGG_GROUP_HAVING_056",
        name: r"AGG_GROUP_HAVING_056",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_056.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 57, 'g1'), (2, 58, 'g2'), (3, 59, 'g0'), (4, 60, 'g1'), (0, 61, 'g2'), (1, 62, 'g0'), (2, 63, 'g1'), (3, 64, 'g2'), (4, 65, 'g0'), (0, 66, 'g1'), (1, 67, 'g2'), (2, 68, 'g0'), (3, 69, 'g1'), (4, 70, 'g2'), (0, 71, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|59|71|65.0
g1|5|10|57|69|63.0
g2|5|10|58|70|64.0
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
