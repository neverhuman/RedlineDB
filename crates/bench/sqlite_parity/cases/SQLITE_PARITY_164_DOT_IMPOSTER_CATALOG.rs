// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_164_DOT_IMPOSTER_CATALOG

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 164,
        folder: r"SQLITE_PARITY_164_DOT_IMPOSTER_CATALOG",
        name: r"DOT_IMPOSTER_CATALOG",
        category: r"CLI_CATALOG",
        priority: r"P4",
        profile: r"catalog",
        kind: r"cli",
        description: r".imposter is unsafe/testing-oriented; catalog entry skipped by default.",
        status: r"catalog_only",
        db: r":memory:",
        args: &[],
        stdin: r".imposter
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Unsafe for normal parity runs; add target-specific guarded case if needed.",
    }
}
