// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_217_DETACH_DATABASE_SYNTAX

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 217,
        folder: r"SQLITE_PARITY_217_DETACH_DATABASE_SYNTAX",
        name: r"DETACH_DATABASE_SYNTAX",
        category: r"SQL_ATTACH",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"DETACH DATABASE syntax after ATTACH DATABASE.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
ATTACH DATABASE ':memory:' AS aux;
CREATE TABLE aux.t(x);
INSERT INTO aux.t VALUES(1);
SELECT x FROM aux.t;
DETACH DATABASE aux;
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
