// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_039_CREATE_TRIGGER_BEFORE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 39,
        folder: r"SQLITE_PARITY_039_CREATE_TRIGGER_BEFORE",
        name: r"CREATE_TRIGGER_BEFORE",
        category: r"SQL_TRIGGER",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"BEFORE INSERT trigger with RAISE(IGNORE).",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x INT);
CREATE TRIGGER tr_bi BEFORE INSERT ON t WHEN NEW.x < 0 BEGIN
  SELECT RAISE(IGNORE);
END;
INSERT INTO t VALUES(-1),(2);
SELECT count(*), min(x) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
