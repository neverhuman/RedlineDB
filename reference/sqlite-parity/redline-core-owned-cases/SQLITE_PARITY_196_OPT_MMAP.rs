// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_196_OPT_MMAP

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 196,
        folder: r"SQLITE_PARITY_196_OPT_MMAP",
        name: r"OPT_MMAP",
        category: r"CLI_OPTION",
        priority: r"P3",
        profile: r"memory",
        kind: r"argv",
        description: r"-mmap smoke.",
        status: r"active",
        db: r":memory:",
        args: &[r"-mmap", r"0", r":memory:", r"SELECT 1;"],
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
