// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_577_AGG_GROUP_HAVING_070

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 577,
        folder: r"SQLITE_PARITY_577_AGG_GROUP_HAVING_070",
        name: r"AGG_GROUP_HAVING_070",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_070.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 71, 'g1'), (2, 72, 'g2'), (3, 73, 'g0'), (4, 74, 'g1'), (0, 75, 'g2'), (1, 76, 'g0'), (2, 77, 'g1'), (3, 78, 'g2'), (4, 79, 'g0'), (0, 80, 'g1'), (1, 81, 'g2'), (2, 82, 'g0'), (3, 83, 'g1'), (4, 84, 'g2'), (0, 85, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|73|85|79.0
g1|5|10|71|83|77.0
g2|5|10|72|84|78.0
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
