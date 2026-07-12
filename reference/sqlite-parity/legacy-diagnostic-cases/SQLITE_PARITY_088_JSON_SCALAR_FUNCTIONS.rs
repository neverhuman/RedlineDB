// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_088_JSON_SCALAR_FUNCTIONS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 88,
        folder: r"SQLITE_PARITY_088_JSON_SCALAR_FUNCTIONS",
        name: r"JSON_SCALAR_FUNCTIONS",
        category: r"SQL_JSON",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"json_valid, json_extract, json_type, json_array_length.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r#".mode list
.headers off
.separator |
.nullvalue NULL
SELECT json_valid('{"a":[1,2]}'),
       json_extract('{"a":[1,2]}','$.a[1]'),
       json_type('{"a":null}','$.a'),
       json_array_length('[1,2,3]');
"#,
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|2|null|3
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
