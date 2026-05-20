// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_007_WITHOUT_ROWID_TABLE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 7,
        folder: r"SQLITE_PARITY_007_WITHOUT_ROWID_TABLE",
        name: r"WITHOUT_ROWID_TABLE",
        category: r"SQL_ROWID",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"WITHOUT ROWID table creation and schema persistence.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a TEXT, b INT, PRIMARY KEY(a,b)) WITHOUT ROWID;
INSERT INTO t VALUES('x',1);
SELECT sql LIKE '%WITHOUT ROWID%' FROM sqlite_schema WHERE name='t';
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
