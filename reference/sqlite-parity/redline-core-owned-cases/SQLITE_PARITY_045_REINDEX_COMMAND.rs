// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_045_REINDEX_COMMAND

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 45,
        folder: r"SQLITE_PARITY_045_REINDEX_COMMAND",
        name: r"REINDEX_COMMAND",
        category: r"SQL_REINDEX",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"REINDEX executes after index creation.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a TEXT COLLATE NOCASE);
CREATE INDEX i_t_a ON t(a);
INSERT INTO t VALUES('A'),('b');
REINDEX;
SELECT count(*) FROM t INDEXED BY i_t_a WHERE a='a';
",
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
