// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_017_DELETE_BASIC

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 17,
        folder: r"SQLITE_PARITY_017_DELETE_BASIC",
        name: r"DELETE_BASIC",
        category: r"SQL_DELETE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"DELETE with WHERE.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(id INT PRIMARY KEY, v TEXT);
INSERT INTO t VALUES(1,'a'),(2,'b'),(3,'c');
DELETE FROM t WHERE id=2;
SELECT group_concat(v,'') FROM t ORDER BY id;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"ac
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
