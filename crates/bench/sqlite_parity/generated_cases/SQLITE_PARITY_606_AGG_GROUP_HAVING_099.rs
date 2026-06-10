// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_606_AGG_GROUP_HAVING_099

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 606,
        folder: r"SQLITE_PARITY_606_AGG_GROUP_HAVING_099",
        name: r"AGG_GROUP_HAVING_099",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_099.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 100, 'g1'), (2, 101, 'g2'), (3, 102, 'g0'), (4, 103, 'g1'), (0, 104, 'g2'), (1, 105, 'g0'), (2, 106, 'g1'), (3, 107, 'g2'), (4, 108, 'g0'), (0, 109, 'g1'), (1, 110, 'g2'), (2, 111, 'g0'), (3, 112, 'g1'), (4, 113, 'g2'), (0, 114, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|102|114|108.0
g1|5|10|100|112|106.0
g2|5|10|101|113|107.0
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
