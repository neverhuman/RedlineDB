// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_029_ALTER_TABLE_RENAME_COLUMN

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 29,
        folder: r"SQLITE_PARITY_029_ALTER_TABLE_RENAME_COLUMN",
        name: r"ALTER_TABLE_RENAME_COLUMN",
        category: r"SQL_ALTER",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"ALTER TABLE RENAME COLUMN.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT);
ALTER TABLE t RENAME COLUMN b TO c;
SELECT name FROM pragma_table_info('t') ORDER BY cid;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"a
c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
