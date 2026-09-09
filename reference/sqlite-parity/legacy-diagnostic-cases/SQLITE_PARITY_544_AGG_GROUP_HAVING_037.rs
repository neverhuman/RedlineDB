// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_544_AGG_GROUP_HAVING_037

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 544,
        folder: r"SQLITE_PARITY_544_AGG_GROUP_HAVING_037",
        name: r"AGG_GROUP_HAVING_037",
        category: r"GEN_SQL_AGGREGATE",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for AGG_GROUP_HAVING_037.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT, g TEXT);
INSERT INTO t VALUES (1, 38, 'g1'), (2, 39, 'g2'), (3, 40, 'g0'), (4, 41, 'g1'), (0, 42, 'g2'), (1, 43, 'g0'), (2, 44, 'g1'), (3, 45, 'g2'), (4, 46, 'g0'), (0, 47, 'g1'), (1, 48, 'g2'), (2, 49, 'g0'), (3, 50, 'g1'), (4, 51, 'g2'), (0, 52, 'g0');
SELECT g, count(*), sum(a), min(b), max(b), round(avg(b),2) FROM t GROUP BY g HAVING count(*) >= 4 ORDER BY g;
SELECT count(DISTINCT a), count(DISTINCT g) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"g0|5|10|40|52|46.0
g1|5|10|38|50|44.0
g2|5|10|39|51|45.0
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
