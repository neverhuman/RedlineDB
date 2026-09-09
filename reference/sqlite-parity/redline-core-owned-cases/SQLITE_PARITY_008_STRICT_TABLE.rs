// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_008_STRICT_TABLE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 8,
        folder: r"SQLITE_PARITY_008_STRICT_TABLE",
        name: r"STRICT_TABLE",
        category: r"SQL_DDL",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"STRICT table creation and strict type storage for valid values.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INTEGER, b TEXT) STRICT;
INSERT INTO t VALUES(1,'ok');
SELECT typeof(a), typeof(b) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"integer|text
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
