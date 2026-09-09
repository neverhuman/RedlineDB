// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_220_DELETE_LIMIT_OPTIONAL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 220,
        folder: r"SQLITE_PARITY_220_DELETE_LIMIT_OPTIONAL",
        name: r"DELETE_LIMIT_OPTIONAL",
        category: r"SQL_DELETE_OPTIONAL",
        priority: r"P3",
        profile: r"memory",
        kind: r"sql",
        description: r"DELETE ... ORDER BY ... LIMIT when compiled with update/delete limit support.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(id INT PRIMARY KEY);
INSERT INTO t VALUES(1),(2),(3);
DELETE FROM t ORDER BY id LIMIT 1;
SELECT group_concat(id,'') FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"23
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
