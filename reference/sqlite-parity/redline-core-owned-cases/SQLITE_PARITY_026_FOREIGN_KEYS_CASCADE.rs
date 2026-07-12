// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_026_FOREIGN_KEYS_CASCADE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 26,
        folder: r"SQLITE_PARITY_026_FOREIGN_KEYS_CASCADE",
        name: r"FOREIGN_KEYS_CASCADE",
        category: r"SQL_FOREIGN_KEYS",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"Foreign keys ON, ON UPDATE CASCADE, ON DELETE CASCADE.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
PRAGMA foreign_keys=ON;
CREATE TABLE p(id INT PRIMARY KEY);
CREATE TABLE c(pid INT REFERENCES p(id) ON UPDATE CASCADE ON DELETE CASCADE);
INSERT INTO p VALUES(1);
INSERT INTO c VALUES(1);
UPDATE p SET id=2 WHERE id=1;
SELECT pid FROM c;
DELETE FROM p WHERE id=2;
SELECT count(*) FROM c;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"2
0
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
