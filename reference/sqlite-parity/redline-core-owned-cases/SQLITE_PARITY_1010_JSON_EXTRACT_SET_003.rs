// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_1010_JSON_EXTRACT_SET_003

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 1010,
        folder: r"SQLITE_PARITY_1010_JSON_EXTRACT_SET_003",
        name: r"JSON_EXTRACT_SET_003",
        category: r"GEN_SQL_JSON",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for JSON_EXTRACT_SET_003.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE docs(id INT PRIMARY KEY, doc TEXT);
INSERT INTO docs VALUES (1, json_object('a',3,'b',json_array(1,2,3)));
INSERT INTO docs VALUES (2, json_set(json_object('a',0), '$.a', 4, '$.c', 'x'));
SELECT id, json_extract(doc,'$.a'), json_type(doc,'$.b'), json_valid(doc) FROM docs ORDER BY id;
SELECT json_array_length(json_extract(doc,'$.b')) FROM docs WHERE id=1;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|3|array|1
2|4|NULL|1
3
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
