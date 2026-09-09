// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_100_SCHEMA_SQLITE_SCHEMA

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 100,
        folder: r"SQLITE_PARITY_100_SCHEMA_SQLITE_SCHEMA",
        name: r"SCHEMA_SQLITE_SCHEMA",
        category: r"SQL_SCHEMA",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"sqlite_schema introspection.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x INT);
SELECT type,name,tbl_name FROM sqlite_schema WHERE name='t';
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"table|t|t
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
