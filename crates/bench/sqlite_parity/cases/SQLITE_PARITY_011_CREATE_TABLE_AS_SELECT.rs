// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_011_CREATE_TABLE_AS_SELECT

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 11,
        folder: r"SQLITE_PARITY_011_CREATE_TABLE_AS_SELECT",
        name: r"CREATE_TABLE_AS_SELECT",
        category: r"SQL_DDL",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"CREATE TABLE AS SELECT and inferred storage classes.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t AS SELECT 1 AS a,'x' AS b;
SELECT a,b,typeof(a),typeof(b) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|x|integer|text
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
