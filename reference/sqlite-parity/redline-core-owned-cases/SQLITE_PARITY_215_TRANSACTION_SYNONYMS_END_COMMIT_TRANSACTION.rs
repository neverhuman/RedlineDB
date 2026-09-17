// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_215_TRANSACTION_SYNONYMS_END_COMMIT_TRANSACTION

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 215,
        folder: r"SQLITE_PARITY_215_TRANSACTION_SYNONYMS_END_COMMIT_TRANSACTION",
        name: r"TRANSACTION_SYNONYMS_END_COMMIT_TRANSACTION",
        category: r"SQL_TRANSACTION",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"BEGIN TRANSACTION, END TRANSACTION, COMMIT TRANSACTION synonyms.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
BEGIN TRANSACTION;
CREATE TABLE t(x);
INSERT INTO t VALUES(1);
END TRANSACTION;
BEGIN TRANSACTION;
INSERT INTO t VALUES(2);
COMMIT TRANSACTION;
SELECT group_concat(x,'') FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"12
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
