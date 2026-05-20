// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_588_AGG_GROUP_HAVING_081

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 588,
        folder: r"SQLITE_PARITY_588_AGG_GROUP_HAVING_081",
        name: r"AGG_GROUP_HAVING_081",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_081.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 82, 'g1'), (2, 83, 'g2'), (3, 84, 'g0'), (4, 85, 'g1'), (0, 86, 'g2'), (1, 87, 'g0'), (2, 88, 'g1'), (3, 89, 'g2'), (4, 90, 'g0'), (0, 91, 'g1'), (1, 92, 'g2'), (2, 93, 'g0'), (3, 94, 'g1'), (4, 95, 'g2'), (0, 96, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|84|96|90.0
g1|5|10|82|94|88.0
g2|5|10|83|95|89.0
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
