// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_954_VIEW_TRIGGER_GENERATED_007

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 954,
        folder: r"SQLITE_PARITY_954_VIEW_TRIGGER_GENERATED_007",
        name: r"VIEW_TRIGGER_GENERATED_007",
        category: r"GEN_SQL_VIEW_TRIGGER",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_007.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(id INTEGER PRIMARY KEY, x INT, y INT GENERATED ALWAYS AS (x*2) VIRTUAL);
CREATE TABLE audit(msg TEXT);
CREATE TRIGGER tr_ai AFTER INSERT ON t BEGIN INSERT INTO audit VALUES ('insert'); END;
INSERT INTO t(id,x) VALUES (1,7),(2,8);
CREATE VIEW v AS SELECT id, y FROM t WHERE y >= 14;
SELECT * FROM v ORDER BY id;
SELECT count(*), group_concat(msg, ',') FROM audit;
DROP VIEW v;
DROP TRIGGER tr_ai;
SELECT count(*) FROM sqlite_schema WHERE name IN ('v','tr_ai');
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|14
2|16
2|insert,insert
0
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
