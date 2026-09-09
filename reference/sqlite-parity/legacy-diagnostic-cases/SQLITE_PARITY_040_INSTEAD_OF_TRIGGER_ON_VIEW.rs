// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_040_INSTEAD_OF_TRIGGER_ON_VIEW

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 40,
        folder: r"SQLITE_PARITY_040_INSTEAD_OF_TRIGGER_ON_VIEW",
        name: r"INSTEAD_OF_TRIGGER_ON_VIEW",
        category: r"SQL_TRIGGER",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"INSTEAD OF INSERT trigger on a view.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE base(x INT);
CREATE VIEW v AS SELECT x FROM base;
CREATE TRIGGER v_ins INSTEAD OF INSERT ON v BEGIN
  INSERT INTO base VALUES(NEW.x);
END;
INSERT INTO v VALUES(9);
SELECT x FROM base;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"9
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
