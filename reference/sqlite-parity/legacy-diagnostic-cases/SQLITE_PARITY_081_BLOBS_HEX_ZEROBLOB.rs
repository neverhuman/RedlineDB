// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_081_BLOBS_HEX_ZEROBLOB

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 81,
        folder: r"SQLITE_PARITY_081_BLOBS_HEX_ZEROBLOB",
        name: r"BLOBS_HEX_ZEROBLOB",
        category: r"SQL_FUNCTIONS",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"BLOB literal, hex(), zeroblob().",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT hex(x'00ff'), length(zeroblob(4)), hex(zeroblob(2));
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"00FF|4|0000
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
