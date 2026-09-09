// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_139_DOT_LINT_FKEY_INDEXES

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 139,
        folder: r"SQLITE_PARITY_139_DOT_LINT_FKEY_INDEXES",
        name: r"DOT_LINT_FKEY_INDEXES",
        category: r"CLI_DOT_COMMAND_DIAGNOSTIC",
        priority: r"P3",
        profile: r"memory",
        kind: r"cli",
        description: r".lint fkey-indexes smoke.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r"PRAGMA foreign_keys=ON;
CREATE TABLE p(id INTEGER PRIMARY KEY);
CREATE TABLE c(pid INT REFERENCES p(id));
.lint fkey-indexes
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"CREATE INDEX"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
