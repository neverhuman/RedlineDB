// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_558_AGG_GROUP_HAVING_051

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 558,
        folder: r"SQLITE_PARITY_558_AGG_GROUP_HAVING_051",
        name: r"AGG_GROUP_HAVING_051",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_051.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 52, 'g1'), (2, 53, 'g2'), (3, 54, 'g0'), (4, 55, 'g1'), (0, 56, 'g2'), (1, 57, 'g0'), (2, 58, 'g1'), (3, 59, 'g2'), (4, 60, 'g0'), (0, 61, 'g1'), (1, 62, 'g2'), (2, 63, 'g0'), (3, 64, 'g1'), (4, 65, 'g2'), (0, 66, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|54|66|60.0
g1|5|10|52|64|58.0
g2|5|10|53|65|59.0
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
