// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_055_JOINS_RIGHT_FULL_OUTER

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 55,
        folder: r"SQLITE_PARITY_055_JOINS_RIGHT_FULL_OUTER",
        name: r"JOINS_RIGHT_FULL_OUTER",
        category: r"SQL_JOIN",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"RIGHT JOIN and FULL OUTER JOIN.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE a(id INT, av TEXT);
CREATE TABLE b(id INT, bv TEXT);
INSERT INTO a VALUES(1,'a1'),(2,'a2');
INSERT INTO b VALUES(2,'b2'),(3,'b3');
SELECT coalesce(a.id,b.id), ifnull(av,'NULL'), bv FROM a RIGHT JOIN b USING(id) ORDER BY 1;
SELECT coalesce(a.id,b.id), ifnull(av,'NULL'), ifnull(bv,'NULL') FROM a FULL OUTER JOIN b USING(id) ORDER BY 1;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"2|a2|b2
3|NULL|b3
1|a1|NULL
2|a2|b2
3|NULL|b3
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
