// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_121_DOT_PARAMETER

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 121,
        folder: r"SQLITE_PARITY_121_DOT_PARAMETER",
        name: r"DOT_PARAMETER",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".parameter init/set/list/clear and named parameter binding.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.parameter init
.parameter set @x 7
SELECT @x, typeof(@x);
.parameter list
.parameter clear
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"7|integer", r"@x"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
