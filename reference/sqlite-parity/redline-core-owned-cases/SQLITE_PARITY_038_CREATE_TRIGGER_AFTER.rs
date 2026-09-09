// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_038_CREATE_TRIGGER_AFTER

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 38,
        folder: r"SQLITE_PARITY_038_CREATE_TRIGGER_AFTER",
        name: r"CREATE_TRIGGER_AFTER",
        category: r"SQL_TRIGGER",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"AFTER INSERT trigger with NEW pseudo-table.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x INT);
CREATE TABLE log(msg TEXT);
CREATE TRIGGER tr_ai AFTER INSERT ON t BEGIN
  INSERT INTO log VALUES('i:'||NEW.x);
END;
INSERT INTO t VALUES(7);
SELECT msg FROM log;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"i:7
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
