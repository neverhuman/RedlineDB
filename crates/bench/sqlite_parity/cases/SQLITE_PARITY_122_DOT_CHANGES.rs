// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_122_DOT_CHANGES

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 122,
        folder: r"SQLITE_PARITY_122_DOT_CHANGES",
        name: r"DOT_CHANGES",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".changes on/off.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".changes on
CREATE TABLE t(x);
INSERT INTO t VALUES(1);
UPDATE t SET x=2;
.changes off
SELECT x FROM t;
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"changes:", r"2"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
