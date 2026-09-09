// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_605_AGG_GROUP_HAVING_098

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 605,
        folder: r"SQLITE_PARITY_605_AGG_GROUP_HAVING_098",
        name: r"AGG_GROUP_HAVING_098",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_098.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 99, 'g1'), (2, 100, 'g2'), (3, 101, 'g0'), (4, 102, 'g1'), (0, 103, 'g2'), (1, 104, 'g0'), (2, 105, 'g1'), (3, 106, 'g2'), (4, 107, 'g0'), (0, 108, 'g1'), (1, 109, 'g2'), (2, 110, 'g0'), (3, 111, 'g1'), (4, 112, 'g2'), (0, 113, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|101|113|107.0
g1|5|10|99|111|105.0
g2|5|10|100|112|106.0
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
