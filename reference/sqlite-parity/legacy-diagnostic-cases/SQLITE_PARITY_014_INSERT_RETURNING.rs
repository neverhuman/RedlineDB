// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_014_INSERT_RETURNING

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 14,
        folder: r"SQLITE_PARITY_014_INSERT_RETURNING",
        name: r"INSERT_RETURNING",
        category: r"SQL_INSERT",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"INSERT ... RETURNING projection.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO t(v) VALUES('a'),('b') RETURNING id,v;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|a
2|b
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
