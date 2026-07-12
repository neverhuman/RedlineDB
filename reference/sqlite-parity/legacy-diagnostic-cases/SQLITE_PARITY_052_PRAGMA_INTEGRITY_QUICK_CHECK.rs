// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_052_PRAGMA_INTEGRITY_QUICK_CHECK

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 52,
        folder: r"SQLITE_PARITY_052_PRAGMA_INTEGRITY_QUICK_CHECK",
        name: r"PRAGMA_INTEGRITY_QUICK_CHECK",
        category: r"SQL_PRAGMA",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"PRAGMA integrity_check and quick_check.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x INT);
INSERT INTO t VALUES(1);
PRAGMA integrity_check;
PRAGMA quick_check;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"ok
ok
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
