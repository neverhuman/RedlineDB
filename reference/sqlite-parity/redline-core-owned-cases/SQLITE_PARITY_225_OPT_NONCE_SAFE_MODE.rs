// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_225_OPT_NONCE_SAFE_MODE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 225,
        folder: r"SQLITE_PARITY_225_OPT_NONCE_SAFE_MODE",
        name: r"OPT_NONCE_SAFE_MODE",
        category: r"CLI_OPTION",
        priority: r"P2",
        profile: r"memory",
        kind: r"argv",
        description: r"-nonce with --safe allows one matching .nonce escape.",
        status: r"active",
        db: r":memory:",
        args: &[r"--safe", r"--nonce", r"n123", r":memory:", r".nonce n123", r#"ATTACH ":memory:" AS aux; SELECT 1;"#],
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
