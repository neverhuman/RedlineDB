// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_147_DOT_IMPORT_CSV_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 147,
        folder: r"SQLITE_PARITY_147_DOT_IMPORT_CSV_TEMPFILE",
        name: r"DOT_IMPORT_CSV_TEMPFILE",
        category: r"CLI_TEMPFILE",
        priority: r"P1",
        profile: r"tempfile",
        kind: r"cli",
        description: r".import CSV from a short-lived temp file.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r"CREATE TABLE t(a INT,b TEXT);
.mode csv
.import --csv {{TMP}}/data.csv t
.mode list
SELECT a,b FROM t ORDER BY a;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|a
2|b
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[crate::FixtureFile { path: r"data.csv", contents: r"1,a
2,b
" }],
        script: None,
        notes: r"",
    }
}
