// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_159_DOT_SYSTEM_SIDE_EFFECT

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 159,
        folder: r"SQLITE_PARITY_159_DOT_SYSTEM_SIDE_EFFECT",
        name: r"DOT_SYSTEM_SIDE_EFFECT",
        category: r"CLI_SIDE_EFFECT",
        priority: r"P4",
        profile: r"side_effect",
        kind: r"cli",
        description: r".system command is excluded by default; enable side-effect profile.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".system printf system-ok
.print done
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"system-ok", r"done"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
