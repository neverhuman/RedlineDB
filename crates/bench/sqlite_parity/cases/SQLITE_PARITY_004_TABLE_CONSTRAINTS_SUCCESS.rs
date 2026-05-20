// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_004_TABLE_CONSTRAINTS_SUCCESS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 4,
        folder: r"SQLITE_PARITY_004_TABLE_CONSTRAINTS_SUCCESS",
        name: r"TABLE_CONSTRAINTS_SUCCESS",
        category: r"SQL_CONSTRAINTS",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"PRIMARY KEY, UNIQUE, NOT NULL, CHECK, DEFAULT success path.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(
  id INTEGER PRIMARY KEY,
  email TEXT UNIQUE NOT NULL,
  score INT CHECK(score BETWEEN 0 AND 100),
  active INT DEFAULT 1
);
INSERT INTO t(email, score) VALUES('a@example.test',10);
SELECT id,email,score,active FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|a@example.test|10|1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
