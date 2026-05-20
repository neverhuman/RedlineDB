// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_094_FTS5_HIGHLIGHT_OPTIONAL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 94,
        folder: r"SQLITE_PARITY_094_FTS5_HIGHLIGHT_OPTIONAL",
        name: r"FTS5_HIGHLIGHT_OPTIONAL",
        category: r"SQL_VIRTUAL_TABLE_OPTIONAL",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql",
        description: r"FTS5 highlight() auxiliary function.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE VIRTUAL TABLE docs USING fts5(title);
INSERT INTO docs(title) VALUES('hello world');
SELECT highlight(docs,0,'[',']') FROM docs WHERE docs MATCH 'hello';
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"[hello] world
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
