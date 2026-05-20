// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_010_GENERATED_COLUMNS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 10,
        folder: r"SQLITE_PARITY_010_GENERATED_COLUMNS",
        name: r"GENERATED_COLUMNS",
        category: r"SQL_DDL",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"STORED and VIRTUAL generated columns.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(
  a INT,
  b INT GENERATED ALWAYS AS (a*2) STORED,
  c INT GENERATED ALWAYS AS (a+1) VIRTUAL
);
INSERT INTO t(a) VALUES(5);
SELECT a,b,c FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"5|10|6
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
