// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_054_JOINS_INNER_LEFT_CROSS_NATURAL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 54,
        folder: r"SQLITE_PARITY_054_JOINS_INNER_LEFT_CROSS_NATURAL",
        name: r"JOINS_INNER_LEFT_CROSS_NATURAL",
        category: r"SQL_JOIN",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"INNER, LEFT, CROSS, NATURAL joins.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE a(id INT, v TEXT);
CREATE TABLE b(id INT, w TEXT);
INSERT INTO a VALUES(1,'a1'),(2,'a2');
INSERT INTO b VALUES(2,'b2'),(3,'b3');
SELECT a.id,a.v,b.w FROM a INNER JOIN b ON a.id=b.id;
SELECT a.id,a.v,ifnull(b.w,'NULL') FROM a LEFT JOIN b ON a.id=b.id ORDER BY a.id;
SELECT count(*) FROM a CROSS JOIN b;
CREATE TABLE n1(k INT, x TEXT);
CREATE TABLE n2(k INT, y TEXT);
INSERT INTO n1 VALUES(5,'x'); INSERT INTO n2 VALUES(5,'y');
SELECT k,x,y FROM n1 NATURAL JOIN n2;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"2|a2|b2
1|a1|NULL
2|a2|b2
4
5|x|y
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
