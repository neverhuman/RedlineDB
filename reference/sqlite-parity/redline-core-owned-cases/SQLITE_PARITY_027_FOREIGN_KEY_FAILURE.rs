// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_027_FOREIGN_KEY_FAILURE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 27,
        folder: r"SQLITE_PARITY_027_FOREIGN_KEY_FAILURE",
        name: r"FOREIGN_KEY_FAILURE",
        category: r"SQL_FOREIGN_KEYS_NEGATIVE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"Foreign-key violation exits non-zero when foreign_keys is ON.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
PRAGMA foreign_keys=ON;
CREATE TABLE p(id INT PRIMARY KEY);
CREATE TABLE c(pid INT REFERENCES p(id));
INSERT INTO c VALUES(99);
",
        expected_exit: 1,
        compare_stdout: true,
        expected_stdout: None,
        expected_stdout_contains: &[],
        expected_stderr_contains: &[r"FOREIGN KEY constraint failed"],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
