// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_075_EXPLAIN_QUERY_PLAN

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 75,
        folder: r"SQLITE_PARITY_075_EXPLAIN_QUERY_PLAN",
        name: r"EXPLAIN_QUERY_PLAN",
        category: r"SQL_EXPLAIN",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"EXPLAIN QUERY PLAN emits a query plan.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT);
CREATE INDEX i_t_a ON t(a);
INSERT INTO t VALUES(1,10),(2,20);
EXPLAIN QUERY PLAN SELECT b FROM t WHERE a=2;
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"i_t_a"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
