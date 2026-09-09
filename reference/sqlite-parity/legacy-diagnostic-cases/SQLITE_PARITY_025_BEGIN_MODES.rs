// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_025_BEGIN_MODES

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 25,
        folder: r"SQLITE_PARITY_025_BEGIN_MODES",
        name: r"BEGIN_MODES",
        category: r"SQL_TRANSACTION",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"BEGIN DEFERRED, IMMEDIATE, EXCLUSIVE.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
BEGIN DEFERRED; COMMIT;
BEGIN IMMEDIATE; COMMIT;
BEGIN EXCLUSIVE; COMMIT;
SELECT 'ok';
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
