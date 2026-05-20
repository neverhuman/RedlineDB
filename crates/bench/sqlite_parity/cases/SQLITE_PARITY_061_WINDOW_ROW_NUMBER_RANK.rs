// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_061_WINDOW_ROW_NUMBER_RANK

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 61,
        folder: r"SQLITE_PARITY_061_WINDOW_ROW_NUMBER_RANK",
        name: r"WINDOW_ROW_NUMBER_RANK",
        category: r"SQL_WINDOW",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"row_number, rank, dense_rank window functions.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(k TEXT, v INT);
INSERT INTO t VALUES('a',10),('a',20),('a',20);
SELECT v,row_number() OVER (ORDER BY v), rank() OVER (ORDER BY v), dense_rank() OVER (ORDER BY v) FROM t ORDER BY v,rowid;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"10|1|1|1
20|2|2|2
20|3|2|2
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
