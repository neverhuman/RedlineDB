// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_019_REPLACE_INTO

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 19,
        folder: r"SQLITE_PARITY_019_REPLACE_INTO",
        name: r"REPLACE_INTO",
        category: r"SQL_REPLACE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"REPLACE INTO conflict behavior.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(k TEXT PRIMARY KEY, v INT);
INSERT INTO t VALUES('a',1);
REPLACE INTO t VALUES('a',2);
SELECT k,v,count(*) OVER () FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"a|2|1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
