// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_211_SQL_ATTACH_TEMPFILE_DATABASE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 211,
        folder: r"SQLITE_PARITY_211_SQL_ATTACH_TEMPFILE_DATABASE",
        name: r"SQL_ATTACH_TEMPFILE_DATABASE",
        category: r"SQL_TEMPFILE",
        priority: r"P1",
        profile: r"tempfile",
        kind: r"sql",
        description: r"ATTACH a short-lived on-disk temp database path; no persistence after runner cleanup.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
ATTACH '{{TMP}}/aux.db' AS aux;
CREATE TABLE aux.t(x INT);
INSERT INTO aux.t VALUES(11);
SELECT x FROM aux.t;
DETACH aux;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"11
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
