// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_076_EXPLAIN_BYTECODE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 76,
        folder: r"SQLITE_PARITY_076_EXPLAIN_BYTECODE",
        name: r"EXPLAIN_BYTECODE",
        category: r"SQL_EXPLAIN",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"EXPLAIN emits virtual-machine bytecode columns/opcodes.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
EXPLAIN SELECT 1;
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"Init", r"ResultRow"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
