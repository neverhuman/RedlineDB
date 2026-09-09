// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_084_CORE_FORMAT_QUOTE_HEX

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 84,
        folder: r"SQLITE_PARITY_084_CORE_FORMAT_QUOTE_HEX",
        name: r"CORE_FORMAT_QUOTE_HEX",
        category: r"SQL_FUNCTIONS",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"printf/format, quote, hex.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT printf('%04d',7), format('%s-%d','x',2), quote('a''b'), hex('AZ');
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0007|x-2|'a''b'|415A
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
