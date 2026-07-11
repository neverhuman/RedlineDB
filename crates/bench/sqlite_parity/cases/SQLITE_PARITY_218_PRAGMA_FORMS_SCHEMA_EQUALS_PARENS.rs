// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_218_PRAGMA_FORMS_SCHEMA_EQUALS_PARENS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 218,
        folder: r"SQLITE_PARITY_218_PRAGMA_FORMS_SCHEMA_EQUALS_PARENS",
        name: r"PRAGMA_FORMS_SCHEMA_EQUALS_PARENS",
        category: r"SQL_PRAGMA",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"PRAGMA schema prefix, equals syntax, and parenthesized syntax.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
PRAGMA main.cache_size = -2000;
PRAGMA main.cache_size;
PRAGMA temp_store(MEMORY);
PRAGMA temp_store;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"-2000
2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
