// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_016_UPDATE_RETURNING

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 16,
        folder: r"SQLITE_PARITY_016_UPDATE_RETURNING",
        name: r"UPDATE_RETURNING",
        category: r"SQL_UPDATE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"UPDATE ... RETURNING.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(id INT PRIMARY KEY, v INT);
INSERT INTO t VALUES(1,10),(2,20);
UPDATE t SET v=v+1 WHERE id=1 RETURNING id,v;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|11
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
