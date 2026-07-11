// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_002_LITERALS_AND_TYPEOF

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 2,
        folder: r"SQLITE_PARITY_002_LITERALS_AND_TYPEOF",
        name: r"LITERALS_AND_TYPEOF",
        category: r"SQL_EXPRESSIONS",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"NULL, integer, real, text, blob literals, hex blobs, quote().",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT typeof(NULL), typeof(1), typeof(1.5), typeof('x'), typeof(x'0102'), hex(x'0102'), quote('a''b');
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"null|integer|real|text|blob|0102|'a''b'
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
