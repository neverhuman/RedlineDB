// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_046_VACUUM_MEMORY

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 46,
        folder: r"SQLITE_PARITY_046_VACUUM_MEMORY",
        name: r"VACUUM_MEMORY",
        category: r"SQL_VACUUM",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"VACUUM on in-memory database plus integrity_check.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x INT);
INSERT INTO t VALUES(1);
VACUUM;
PRAGMA integrity_check;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"ok
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
