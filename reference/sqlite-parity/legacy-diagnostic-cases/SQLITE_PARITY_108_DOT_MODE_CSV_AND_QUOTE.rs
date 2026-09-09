// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_108_DOT_MODE_CSV_AND_QUOTE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 108,
        folder: r"SQLITE_PARITY_108_DOT_MODE_CSV_AND_QUOTE",
        name: r"DOT_MODE_CSV_AND_QUOTE",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".mode csv and .mode quote output formats.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r"CREATE TABLE t(a,b);
INSERT INTO t VALUES(1,'x,y');
.mode csv
SELECT * FROM t;
.mode quote
SELECT * FROM t;
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r#"1,"x,y""#, r"1,'x,y'"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
