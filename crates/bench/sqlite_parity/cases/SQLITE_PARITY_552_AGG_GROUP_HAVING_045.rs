// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_552_AGG_GROUP_HAVING_045

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 552,
        folder: r"SQLITE_PARITY_552_AGG_GROUP_HAVING_045",
        name: r"AGG_GROUP_HAVING_045",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_045.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 46, 'g1'), (2, 47, 'g2'), (3, 48, 'g0'), (4, 49, 'g1'), (0, 50, 'g2'), (1, 51, 'g0'), (2, 52, 'g1'), (3, 53, 'g2'), (4, 54, 'g0'), (0, 55, 'g1'), (1, 56, 'g2'), (2, 57, 'g0'), (3, 58, 'g1'), (4, 59, 'g2'), (0, 60, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|48|60|54.0
g1|5|10|46|58|52.0
g2|5|10|47|59|53.0
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
