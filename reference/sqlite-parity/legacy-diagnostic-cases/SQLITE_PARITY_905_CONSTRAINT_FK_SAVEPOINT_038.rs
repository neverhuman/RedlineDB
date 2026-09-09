// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_905_CONSTRAINT_FK_SAVEPOINT_038

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 905,
        folder: r"SQLITE_PARITY_905_CONSTRAINT_FK_SAVEPOINT_038",
        name: r"CONSTRAINT_FK_SAVEPOINT_038",
        category: r"GEN_SQL_CONSTRAINT_TX",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_038.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
PRAGMA foreign_keys=ON;
CREATE TABLE parent(id INT PRIMARY KEY);
CREATE TABLE child(id INT PRIMARY KEY, parent_id INT REFERENCES parent(id) ON DELETE CASCADE, u TEXT UNIQUE);
INSERT INTO parent VALUES (1),(2),(3);
INSERT INTO child VALUES (1,1,'a'),(2,2,'b');
INSERT OR IGNORE INTO child VALUES (1,1,'dup_ignored');
INSERT INTO child VALUES (4,3,'c') ON CONFLICT(id) DO UPDATE SET u='updated';
SAVEPOINT s1;
INSERT OR REPLACE INTO child VALUES (2,2,'b2');
ROLLBACK TO s1;
RELEASE s1;
DELETE FROM parent WHERE id=3;
SELECT id, parent_id, u FROM child ORDER BY id;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|1|a
2|2|b
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
