// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_003_CREATE_TABLE_INSERT_SELECT

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 3,
        folder: r"SQLITE_PARITY_003_CREATE_TABLE_INSERT_SELECT",
        name: r"CREATE_TABLE_INSERT_SELECT",
        category: r"SQL_DDL_DML",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"Basic CREATE TABLE, multi-row INSERT, SELECT aggregate count/group_concat.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INTEGER, b TEXT);
INSERT INTO t VALUES (1,'one'),(2,'two');
SELECT count(*), group_concat(b, ',') FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"2|one,two
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
