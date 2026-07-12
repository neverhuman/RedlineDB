// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_013_INSERT_SELECT

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 13,
        folder: r"SQLITE_PARITY_013_INSERT_SELECT",
        name: r"INSERT_SELECT",
        category: r"SQL_INSERT",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"INSERT INTO ... SELECT.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b TEXT);
INSERT INTO t SELECT 1,'a' UNION ALL SELECT 2,'b';
SELECT group_concat(a||b, ',') FROM t ORDER BY a;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1a,2b
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
