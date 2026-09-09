// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_115_DOT_SCHEMA_TABLES_INDEXES

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 115,
        folder: r"SQLITE_PARITY_115_DOT_SCHEMA_TABLES_INDEXES",
        name: r"DOT_SCHEMA_TABLES_INDEXES",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".schema, .tables, .indexes.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r"CREATE TABLE t(a INT);
CREATE INDEX idx_t_a ON t(a);
.tables
.indexes t
.schema t
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"t", r"idx_t_a", r"CREATE TABLE t"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
