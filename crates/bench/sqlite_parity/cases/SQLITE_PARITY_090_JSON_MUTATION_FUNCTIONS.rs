// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_090_JSON_MUTATION_FUNCTIONS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 90,
        folder: r"SQLITE_PARITY_090_JSON_MUTATION_FUNCTIONS",
        name: r"JSON_MUTATION_FUNCTIONS",
        category: r"SQL_JSON",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"json_set, json_remove, json_patch.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r#".mode list
.headers off
.separator |
.nullvalue NULL
SELECT json_set('{"a":1}','$.b',2),
       json_remove('{"a":1,"b":2}','$.b'),
       json_patch('{"a":1}','{"b":2}');
"#,
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r#"{"a":1,"b":2}|{"a":1}|{"a":1,"b":2}
"#),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
