// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_138_DOT_VFSNAME_LIST_INFO

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 138,
        folder: r"SQLITE_PARITY_138_DOT_VFSNAME_LIST_INFO",
        name: r"DOT_VFSNAME_LIST_INFO",
        category: r"CLI_DOT_COMMAND_DIAGNOSTIC",
        priority: r"P3",
        profile: r"memory",
        kind: r"cli",
        description: r".vfsname, .vfslist, .vfsinfo smoke.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".vfsname
.vfslist
.vfsinfo
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"vfs"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
