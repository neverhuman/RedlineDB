// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_545_AGG_GROUP_HAVING_038

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 545,
        folder: r"SQLITE_PARITY_545_AGG_GROUP_HAVING_038",
        name: r"AGG_GROUP_HAVING_038",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_038.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 39, 'g1'), (2, 40, 'g2'), (3, 41, 'g0'), (4, 42, 'g1'), (0, 43, 'g2'), (1, 44, 'g0'), (2, 45, 'g1'), (3, 46, 'g2'), (4, 47, 'g0'), (0, 48, 'g1'), (1, 49, 'g2'), (2, 50, 'g0'), (3, 51, 'g1'), (4, 52, 'g2'), (0, 53, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|41|53|47.0
g1|5|10|39|51|45.0
g2|5|10|40|52|46.0
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
