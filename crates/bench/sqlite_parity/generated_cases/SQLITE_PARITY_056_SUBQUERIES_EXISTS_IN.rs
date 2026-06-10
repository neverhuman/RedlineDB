// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_056_SUBQUERIES_EXISTS_IN

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 56,
        folder: r"SQLITE_PARITY_056_SUBQUERIES_EXISTS_IN",
        name: r"SUBQUERIES_EXISTS_IN",
        category: r"SQL_SELECT",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"Scalar subquery, EXISTS, IN.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x INT);
INSERT INTO t VALUES(1),(2),(3);
SELECT (SELECT max(x) FROM t), EXISTS(SELECT 1 FROM t WHERE x=2), 3 IN (SELECT x FROM t);
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"3|1|1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
