// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_006_ROWID_INTEGER_PRIMARY_KEY

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 6,
        folder: r"SQLITE_PARITY_006_ROWID_INTEGER_PRIMARY_KEY",
        name: r"ROWID_INTEGER_PRIMARY_KEY",
        category: r"SQL_ROWID",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"ROWID aliasing through INTEGER PRIMARY KEY.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO t(v) VALUES('a'),('b');
SELECT id,rowid,v FROM t ORDER BY id;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|1|a
2|2|b
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
