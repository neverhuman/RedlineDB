// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_546_AGG_GROUP_HAVING_039

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 546,
        folder: r"SQLITE_PARITY_546_AGG_GROUP_HAVING_039",
        name: r"AGG_GROUP_HAVING_039",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_039.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 40, 'g1'), (2, 41, 'g2'), (3, 42, 'g0'), (4, 43, 'g1'), (0, 44, 'g2'), (1, 45, 'g0'), (2, 46, 'g1'), (3, 47, 'g2'), (4, 48, 'g0'), (0, 49, 'g1'), (1, 50, 'g2'), (2, 51, 'g0'), (3, 52, 'g1'), (4, 53, 'g2'), (0, 54, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|42|54|48.0
g1|5|10|40|52|46.0
g2|5|10|41|53|47.0
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
