// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_1075_INDEX_SCHEMA_PRAGMA_008

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 1075,
        folder: r"SQLITE_PARITY_1075_INDEX_SCHEMA_PRAGMA_008",
        name: r"INDEX_SCHEMA_PRAGMA_008",
        category: r"GEN_SQL_INDEX_PRAGMA",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_008.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT, b TEXT, c INT);
INSERT INTO t VALUES (1,'one',8),(2,'two',9),(3,'three',10),(4,'four',11);
CREATE INDEX idx_t_a_c ON t(a, c);
CREATE UNIQUE INDEX idx_t_b ON t(b);
SELECT name, type FROM sqlite_schema WHERE tbl_name='t' ORDER BY name;
SELECT a, c FROM t INDEXED BY idx_t_a_c WHERE a >= 2 ORDER BY a;
PRAGMA table_info(t);
PRAGMA index_list(t);
DROP INDEX idx_t_a_c;
SELECT count(*) FROM sqlite_schema WHERE name='idx_t_a_c';
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"idx_t_a_c|index
idx_t_b|index
t|table
2|9
3|10
4|11
0|a|INT|0|NULL|0
1|b|TEXT|0|NULL|0
2|c|INT|0|NULL|0
0|idx_t_b|1|c|0
1|idx_t_a_c|0|c|0
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
