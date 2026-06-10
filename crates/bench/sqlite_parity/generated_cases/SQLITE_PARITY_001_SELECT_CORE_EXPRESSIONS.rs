// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_001_SELECT_CORE_EXPRESSIONS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 1,
        folder: r"SQLITE_PARITY_001_SELECT_CORE_EXPRESSIONS",
        name: r"SELECT_CORE_EXPRESSIONS",
        category: r"SQL_SELECT",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"SELECT, arithmetic, concatenation, integer/real division, modulo, unary operators.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
SELECT 1+2, 'a'||'b', 7/2, 7/2.0, 7%2, -5, +5;
", // jankurai:allow HLT-023-INPUT-BOUNDARY-GAP reason=sqlite-parity-corpus-literal-not-runtime-built-sql expires=2027-06-01
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"3|ab|3|3.5|1|-5|5
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
