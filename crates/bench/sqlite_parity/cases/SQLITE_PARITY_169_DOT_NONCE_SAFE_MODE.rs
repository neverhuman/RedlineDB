// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_169_DOT_NONCE_SAFE_MODE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 169,
        folder: r"SQLITE_PARITY_169_DOT_NONCE_SAFE_MODE",
        name: r"DOT_NONCE_SAFE_MODE",
        category: r"CLI_DOT_COMMAND",
        priority: r"P2",
        profile: r"memory",
        kind: r"argv",
        description: r".nonce with --safe escape nonce for one command.",
        status: r"active",
        db: r":memory:",
        args: &[r"--safe", r"--nonce", r"abc123", r":memory:", r".nonce abc123", r"SELECT 1;"],
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
