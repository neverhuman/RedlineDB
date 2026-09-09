// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_569_AGG_GROUP_HAVING_062

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 569,
        folder: r"SQLITE_PARITY_569_AGG_GROUP_HAVING_062",
        name: r"AGG_GROUP_HAVING_062",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_062.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 63, 'g1'), (2, 64, 'g2'), (3, 65, 'g0'), (4, 66, 'g1'), (0, 67, 'g2'), (1, 68, 'g0'), (2, 69, 'g1'), (3, 70, 'g2'), (4, 71, 'g0'), (0, 72, 'g1'), (1, 73, 'g2'), (2, 74, 'g0'), (3, 75, 'g1'), (4, 76, 'g2'), (0, 77, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|65|77|71.0
g1|5|10|63|75|69.0
g2|5|10|64|76|70.0
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
