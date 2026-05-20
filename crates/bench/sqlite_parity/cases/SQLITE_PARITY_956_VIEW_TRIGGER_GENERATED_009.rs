// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_956_VIEW_TRIGGER_GENERATED_009

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 956,
        folder: r"SQLITE_PARITY_956_VIEW_TRIGGER_GENERATED_009",
        name: r"VIEW_TRIGGER_GENERATED_009",
        category: r"GEN_SQL_VIEW_TRIGGER",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_009.",
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
INSERT INTO t(id,x) VALUES (1,9),(2,10);
CREATE VIEW v AS SELECT id, y FROM t WHERE y >= 18;
SELECT * FROM v ORDER BY id;
SELECT count(*), group_concat(msg, ',') FROM audit;
DROP VIEW v;
DROP TRIGGER tr_ai;
SELECT count(*) FROM sqlite_schema WHERE name IN ('v','tr_ai');
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|18
2|20
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
