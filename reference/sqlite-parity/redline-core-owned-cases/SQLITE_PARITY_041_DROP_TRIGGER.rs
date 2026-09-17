// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_041_DROP_TRIGGER

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 41,
        folder: r"SQLITE_PARITY_041_DROP_TRIGGER",
        name: r"DROP_TRIGGER",
        category: r"SQL_DROP",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"DROP TRIGGER removes trigger from schema.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x INT);
CREATE TRIGGER tr AFTER INSERT ON t BEGIN SELECT 1; END;
DROP TRIGGER tr;
SELECT count(*) FROM sqlite_schema WHERE type='trigger' AND name='tr';
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"0
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
