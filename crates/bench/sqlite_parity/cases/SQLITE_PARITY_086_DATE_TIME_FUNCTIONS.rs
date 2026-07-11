// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_086_DATE_TIME_FUNCTIONS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 86,
        folder: r"SQLITE_PARITY_086_DATE_TIME_FUNCTIONS",
        name: r"DATE_TIME_FUNCTIONS",
        category: r"SQL_FUNCTIONS",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"date, time, datetime, unixepoch, strftime on fixed inputs.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT date('2024-02-29','+1 day'),
       time('2000-01-01 12:34:56'),
       datetime('2024-01-01 01:02:03'),
       unixepoch('1970-01-02'),
       strftime('%Y','2000-01-01');
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"2024-03-01|12:34:56|2024-01-01 01:02:03|86400|2000
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
