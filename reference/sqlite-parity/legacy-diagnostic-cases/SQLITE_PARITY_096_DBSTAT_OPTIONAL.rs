// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_096_DBSTAT_OPTIONAL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 96,
        folder: r"SQLITE_PARITY_096_DBSTAT_OPTIONAL",
        name: r"DBSTAT_OPTIONAL",
        category: r"SQL_VIRTUAL_TABLE_OPTIONAL",
        priority: r"P3",
        profile: r"memory",
        kind: r"sql",
        description: r"dbstat virtual table when compiled/enabled.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x);
INSERT INTO t VALUES(1);
CREATE VIRTUAL TABLE temp.stat USING dbstat;
SELECT count(*)>0 FROM stat;
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
