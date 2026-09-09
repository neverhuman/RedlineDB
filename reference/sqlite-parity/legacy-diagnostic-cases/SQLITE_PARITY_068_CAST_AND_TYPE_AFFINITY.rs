// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_068_CAST_AND_TYPE_AFFINITY

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 68,
        folder: r"SQLITE_PARITY_068_CAST_AND_TYPE_AFFINITY",
        name: r"CAST_AND_TYPE_AFFINITY",
        category: r"SQL_EXPRESSIONS",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"CAST and storage affinity conversions.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INTEGER, b TEXT, c REAL);
INSERT INTO t VALUES('5', 7, '2.5');
SELECT typeof(a),typeof(b),typeof(c), CAST('10' AS INT)+1 FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"integer|text|real|11
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
