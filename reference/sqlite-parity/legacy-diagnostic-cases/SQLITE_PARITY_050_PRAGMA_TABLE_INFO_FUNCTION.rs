// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_050_PRAGMA_TABLE_INFO_FUNCTION

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 50,
        folder: r"SQLITE_PARITY_050_PRAGMA_TABLE_INFO_FUNCTION",
        name: r"PRAGMA_TABLE_INFO_FUNCTION",
        category: r"SQL_PRAGMA",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"Table-valued PRAGMA function pragma_table_info().",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r#".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT NOT NULL PRIMARY KEY, b TEXT NOT NULL DEFAULT 'x');
SELECT name,type,"notnull",dflt_value,pk FROM pragma_table_info('t') ORDER BY cid;
"#,
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"a|INT|1|NULL|1
b|TEXT|1|'x'|0
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
