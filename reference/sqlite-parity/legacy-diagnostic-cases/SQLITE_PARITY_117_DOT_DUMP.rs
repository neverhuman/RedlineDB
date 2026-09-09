// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_117_DOT_DUMP

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 117,
        folder: r"SQLITE_PARITY_117_DOT_DUMP",
        name: r"DOT_DUMP",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".dump renders SQL for content.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r"CREATE TABLE t(x INT);
INSERT INTO t VALUES(1);
.dump
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"CREATE TABLE t", r"INSERT INTO t VALUES(1)"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
