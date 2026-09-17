// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_093_CREATE_VIRTUAL_TABLE_FTS5_OPTIONAL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 93,
        folder: r"SQLITE_PARITY_093_CREATE_VIRTUAL_TABLE_FTS5_OPTIONAL",
        name: r"CREATE_VIRTUAL_TABLE_FTS5_OPTIONAL",
        category: r"SQL_VIRTUAL_TABLE_OPTIONAL",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql",
        description: r"CREATE VIRTUAL TABLE USING fts5 and MATCH.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE VIRTUAL TABLE docs USING fts5(title, body);
INSERT INTO docs(title,body) VALUES('one','hello world'),('two','other text');
SELECT rowid,title FROM docs WHERE docs MATCH 'hello';
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|one
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
