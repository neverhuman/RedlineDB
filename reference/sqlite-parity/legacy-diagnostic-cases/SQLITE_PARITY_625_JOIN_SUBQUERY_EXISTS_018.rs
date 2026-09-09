// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_625_JOIN_SUBQUERY_EXISTS_018

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 625,
        folder: r"SQLITE_PARITY_625_JOIN_SUBQUERY_EXISTS_018",
        name: r"JOIN_SUBQUERY_EXISTS_018",
        category: r"GEN_SQL_JOIN_SUBQUERY",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_018.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE a(id INT PRIMARY KEY, x TEXT);
CREATE TABLE b(id INT PRIMARY KEY, a_id INT, y TEXT);
INSERT INTO a VALUES (1,'a1'),(2,'a2'),(3,'a3'),(4,'a4');
INSERT INTO b VALUES (1,1,'b1'),(2,1,'b2'),(3,3,'b3'),(4,3,'bn');
SELECT a.id, a.x, b.y FROM a JOIN b ON b.a_id=a.id WHERE a.id IN (SELECT a_id FROM b WHERE id <= 4) ORDER BY a.id, b.y;
SELECT a.id, EXISTS(SELECT 1 FROM b WHERE b.a_id=a.id AND b.y='bn') FROM a ORDER BY a.id;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|a1|b1
1|a1|b2
3|a3|b3
3|a3|bn
1|0
2|0
3|1
4|0
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
