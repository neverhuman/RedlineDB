// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_009_STRICT_TABLE_TYPE_FAILURE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 9,
        folder: r"SQLITE_PARITY_009_STRICT_TABLE_TYPE_FAILURE",
        name: r"STRICT_TABLE_TYPE_FAILURE",
        category: r"SQL_DDL_NEGATIVE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"STRICT table rejects invalid storage class.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INTEGER) STRICT;
INSERT INTO t VALUES('not-an-int');
",
        expected_exit: 1,
        compare_stdout: true,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[r"cannot store TEXT value in INTEGER column"],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
