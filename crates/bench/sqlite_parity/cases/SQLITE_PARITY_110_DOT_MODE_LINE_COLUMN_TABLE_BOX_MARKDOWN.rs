// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_110_DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 110,
        folder: r"SQLITE_PARITY_110_DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN",
        name: r"DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN",
        category: r"CLI_DOT_COMMAND",
        priority: r"P0",
        profile: r"memory",
        kind: r"cli",
        description: r".mode line/column/table/box/markdown smoke.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode line
SELECT 1 AS a, 'x' AS b;
.mode column
SELECT 1 AS a, 'x' AS b;
.mode table
SELECT 1 AS a, 'x' AS b;
.mode box
SELECT 1 AS a, 'x' AS b;
.mode markdown
SELECT 1 AS a, 'x' AS b;
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"a = 1", r"x", r"|"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
