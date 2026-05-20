// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_089_JSON_TABLE_VALUED_FUNCTIONS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 89,
        folder: r"SQLITE_PARITY_089_JSON_TABLE_VALUED_FUNCTIONS",
        name: r"JSON_TABLE_VALUED_FUNCTIONS",
        category: r"SQL_JSON",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"json_each table-valued function.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r#".mode list
.headers off
.separator |
.nullvalue NULL
SELECT key,value FROM json_each('{"a":1,"b":2}') ORDER BY key;
"#,
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"a|1
b|2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
