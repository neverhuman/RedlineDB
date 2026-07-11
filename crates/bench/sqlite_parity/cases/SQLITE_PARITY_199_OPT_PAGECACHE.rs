// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_199_OPT_PAGECACHE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 199,
        folder: r"SQLITE_PARITY_199_OPT_PAGECACHE",
        name: r"OPT_PAGECACHE",
        category: r"CLI_OPTION",
        priority: r"P3",
        profile: r"memory",
        kind: r"argv",
        description: r"-pagecache smoke.",
        status: r"active",
        db: r":memory:",
        args: &[r"-pagecache", r"1024", r"4", r":memory:", r"SELECT 1;"],
        stdin: r"",
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
