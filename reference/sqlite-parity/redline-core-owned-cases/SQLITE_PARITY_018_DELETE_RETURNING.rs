// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_018_DELETE_RETURNING

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 18,
        folder: r"SQLITE_PARITY_018_DELETE_RETURNING",
        name: r"DELETE_RETURNING",
        category: r"SQL_DELETE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"DELETE ... RETURNING.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(id INT PRIMARY KEY, v TEXT);
INSERT INTO t VALUES(1,'a'),(2,'b');
DELETE FROM t WHERE id=2 RETURNING id,v;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"2|b
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
