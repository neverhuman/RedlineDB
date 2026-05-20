// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_036_CREATE_VIEW

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 36,
        folder: r"SQLITE_PARITY_036_CREATE_VIEW",
        name: r"CREATE_VIEW",
        category: r"SQL_VIEW",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"CREATE VIEW and read-through SELECT.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b INT);
INSERT INTO t VALUES(1,10),(2,20);
CREATE VIEW v AS SELECT a,b*2 AS bb FROM t WHERE a=2;
SELECT * FROM v;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"2|40
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
