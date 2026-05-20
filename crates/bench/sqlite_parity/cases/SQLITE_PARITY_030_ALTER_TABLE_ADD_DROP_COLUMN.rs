// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_030_ALTER_TABLE_ADD_DROP_COLUMN

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 30,
        folder: r"SQLITE_PARITY_030_ALTER_TABLE_ADD_DROP_COLUMN",
        name: r"ALTER_TABLE_ADD_DROP_COLUMN",
        category: r"SQL_ALTER",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"ALTER TABLE ADD COLUMN and DROP COLUMN.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT,b INT);
ALTER TABLE t ADD COLUMN c TEXT DEFAULT 'd';
ALTER TABLE t DROP COLUMN b;
INSERT INTO t(a) VALUES (1);
SELECT a,c FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|d
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
