// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_043_ATTACH_DETACH_MEMORY

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 43,
        folder: r"SQLITE_PARITY_043_ATTACH_DETACH_MEMORY",
        name: r"ATTACH_DETACH_MEMORY",
        category: r"SQL_ATTACH",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"ATTACH ':memory:' AS aux and DETACH.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
ATTACH ':memory:' AS aux;
CREATE TABLE aux.t(x INT);
INSERT INTO aux.t VALUES(1);
SELECT x FROM aux.t;
DETACH aux;
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
