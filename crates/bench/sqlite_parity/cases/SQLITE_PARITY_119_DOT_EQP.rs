// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_119_DOT_EQP

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 119,
        folder: r"SQLITE_PARITY_119_DOT_EQP",
        name: r"DOT_EQP",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".eqp on emits query plan.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".eqp on
CREATE TABLE t(a INT);
INSERT INTO t VALUES(1),(2);
SELECT * FROM t WHERE a=2;
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"QUERY PLAN", r"2"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
