// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_589_AGG_GROUP_HAVING_082

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 589,
        folder: r"SQLITE_PARITY_589_AGG_GROUP_HAVING_082",
        name: r"AGG_GROUP_HAVING_082",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_082.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 83, 'g1'), (2, 84, 'g2'), (3, 85, 'g0'), (4, 86, 'g1'), (0, 87, 'g2'), (1, 88, 'g0'), (2, 89, 'g1'), (3, 90, 'g2'), (4, 91, 'g0'), (0, 92, 'g1'), (1, 93, 'g2'), (2, 94, 'g0'), (3, 95, 'g1'), (4, 96, 'g2'), (0, 97, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|85|97|91.0
g1|5|10|83|95|89.0
g2|5|10|84|96|90.0
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
