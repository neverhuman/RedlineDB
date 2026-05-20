// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_012_INSERT_DEFAULT_VALUES

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 12,
        folder: r"SQLITE_PARITY_012_INSERT_DEFAULT_VALUES",
        name: r"INSERT_DEFAULT_VALUES",
        category: r"SQL_INSERT",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"INSERT DEFAULT VALUES with DEFAULT expressions.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT DEFAULT 7, b TEXT DEFAULT 'x');
INSERT INTO t DEFAULT VALUES;
SELECT a,b FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"7|x
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
