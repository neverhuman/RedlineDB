// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_073_INDEXED_BY

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 73,
        folder: r"SQLITE_PARITY_073_INDEXED_BY",
        name: r"INDEXED_BY",
        category: r"SQL_INDEX",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"INDEXED BY clause.",
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
SELECT b FROM t INDEXED BY i_t_a WHERE a=2;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"20
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
