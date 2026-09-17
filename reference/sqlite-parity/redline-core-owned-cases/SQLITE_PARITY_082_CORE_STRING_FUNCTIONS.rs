// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_082_CORE_STRING_FUNCTIONS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 82,
        folder: r"SQLITE_PARITY_082_CORE_STRING_FUNCTIONS",
        name: r"CORE_STRING_FUNCTIONS",
        category: r"SQL_FUNCTIONS",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"substr, replace, upper, lower, trim, instr.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT substr('abcdef',2,3), replace('abc','b','X'), upper('a'), lower('Z'), trim(' x '), instr('abc','b');
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"bcd|aXc|A|z|x|2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
