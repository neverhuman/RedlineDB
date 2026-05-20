// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_015_UPDATE_BASIC

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 15,
        folder: r"SQLITE_PARITY_015_UPDATE_BASIC",
        name: r"UPDATE_BASIC",
        category: r"SQL_UPDATE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"UPDATE with WHERE and subsequent SELECT.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(id INT PRIMARY KEY, v INT);
INSERT INTO t VALUES(1,10),(2,20);
UPDATE t SET v=v+5 WHERE id=2;
SELECT id,v FROM t ORDER BY id;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|10
2|25
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
