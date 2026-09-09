// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_130_DOT_OPEN_MEMORY

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 130,
        folder: r"SQLITE_PARITY_130_DOT_OPEN_MEMORY",
        name: r"DOT_OPEN_MEMORY",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".open :memory: no persistent DB.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".open :memory:
CREATE TABLE t(x);
INSERT INTO t VALUES(1);
SELECT x FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
