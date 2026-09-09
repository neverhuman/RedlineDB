// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_145_DOT_SCANSTATS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 145,
        folder: r"SQLITE_PARITY_145_DOT_SCANSTATS",
        name: r"DOT_SCANSTATS",
        category: r"CLI_DOT_COMMAND_DIAGNOSTIC",
        priority: r"P3",
        profile: r"memory",
        kind: r"cli",
        description: r".scanstats smoke when available.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".scanstats on
CREATE TABLE t(x);
INSERT INTO t VALUES(1);
SELECT * FROM t;
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"1"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
