// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_205_OPT_VFS_CATALOG

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 205,
        folder: r"SQLITE_PARITY_205_OPT_VFS_CATALOG",
        name: r"OPT_VFS_CATALOG",
        category: r"CLI_OPTION_CATALOG",
        priority: r"P4",
        profile: r"catalog",
        kind: r"argv",
        description: r"-vfs is platform-specific; catalog-only by default.",
        status: r"catalog_only",
        db: r":memory:",
        args: &[r"-vfs", r"unix", r":memory:", r"SELECT 1;"],
        stdin: r"",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
