// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_140_DOT_EXPERT_OPTIONAL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 140,
        folder: r"SQLITE_PARITY_140_DOT_EXPERT_OPTIONAL",
        name: r"DOT_EXPERT_OPTIONAL",
        category: r"CLI_DOT_COMMAND_OPTIONAL",
        priority: r"P3",
        profile: r"memory",
        kind: r"cli",
        description: r".expert index recommendation smoke.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r"CREATE TABLE t(a,b);
.expert
SELECT * FROM t WHERE a=1;
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"CREATE INDEX", r"SEARCH"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
