// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_024_SAVEPOINT_ROLLBACK_RELEASE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 24,
        folder: r"SQLITE_PARITY_024_SAVEPOINT_ROLLBACK_RELEASE",
        name: r"SAVEPOINT_ROLLBACK_RELEASE",
        category: r"SQL_SAVEPOINT",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"SAVEPOINT, ROLLBACK TO, RELEASE nested savepoint behavior.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x INT);
SAVEPOINT s1;
INSERT INTO t VALUES(1);
SAVEPOINT s2;
INSERT INTO t VALUES(2);
ROLLBACK TO s2;
RELEASE s2;
RELEASE s1;
SELECT group_concat(x,'') FROM t;
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
