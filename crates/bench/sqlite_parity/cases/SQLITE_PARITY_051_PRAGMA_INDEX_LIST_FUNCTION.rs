// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_051_PRAGMA_INDEX_LIST_FUNCTION

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 51,
        folder: r"SQLITE_PARITY_051_PRAGMA_INDEX_LIST_FUNCTION",
        name: r"PRAGMA_INDEX_LIST_FUNCTION",
        category: r"SQL_PRAGMA",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"Table-valued PRAGMA function pragma_index_list().",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT);
CREATE INDEX i_t_a ON t(a);
SELECT name,origin FROM pragma_index_list('t') ORDER BY name;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"i_t_a|c
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
