//! Subprocess-based parity tests for the `.`-prefixed shell commands.
//!
//! Each test spawns the `redlinedb` CLI binary, drives it with a script on
//! stdin (`-batch` mode), and asserts on stdout/stderr. The legacy SQLite3
//! binary is required for the `.dump` round-trip test that pipes output
//! through `sqlite3 :memory:`.
//!
//! The `redlinedb` binary is located via `assert_cmd::cargo::cargo_bin`.

use std::process::Command;

use assert_cmd::cargo::cargo_bin;
use tempfile::tempdir;

fn run_script(db: Option<&std::path::Path>, script: &str) -> (String, String, i32) {
    run_script_with_args(&[], db, script)
}

fn run_script_with_args(
    extra_args: &[&str],
    db: Option<&std::path::Path>,
    script: &str,
) -> (String, String, i32) {
    let bin = cargo_bin("redlinedb-cli");
    let mut cmd = Command::new(bin);
    cmd.arg("-batch");
    for arg in extra_args {
        cmd.arg(arg);
    }
    if let Some(path) = db {
        cmd.arg(path);
    } else {
        cmd.arg(":memory:");
    }
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("spawn redlinedb cli");
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().expect("stdin");
        stdin.write_all(script.as_bytes()).expect("write stdin");
    }
    let output = child.wait_with_output().expect("wait redlinedb cli");
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

fn sqlite3_version() -> Option<String> {
    let output = match Command::new("sqlite3").arg("--version").output() {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
        Err(err) => panic!("failed to invoke sqlite3: {err}"),
    };
    assert!(
        output.status.success(),
        "sqlite3 --version failed: stderr={:?}",
        output.stderr
    );
    Some(String::from_utf8(output.stdout).expect("sqlite3 version utf8"))
}

#[test]
fn dot_tables_lists_user_tables() {
    let (out, err, code) = run_script(
        None,
        "CREATE TABLE alpha(id INTEGER);\n\
         CREATE TABLE bravo(id INTEGER);\n\
         .tables\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("alpha"), "stdout={out}");
    assert!(out.contains("bravo"), "stdout={out}");
}

#[test]
fn dot_tables_pattern_filters_results() {
    let (out, _err, code) = run_script(
        None,
        "CREATE TABLE alpha(id INTEGER);\n\
         CREATE TABLE bravo(id INTEGER);\n\
         .tables br%\n",
    );
    assert_eq!(code, 0);
    assert!(!out.contains("alpha"), "stdout={out}");
    assert!(out.contains("bravo"), "stdout={out}");
}

#[test]
fn dot_schema_prints_create_statements() {
    let (out, _err, code) = run_script(
        None,
        "CREATE TABLE widgets(id INTEGER PRIMARY KEY, name TEXT);\n\
         .schema widgets\n",
    );
    assert_eq!(code, 0);
    let lower = out.to_ascii_lowercase();
    assert!(lower.contains("create table"), "stdout={out}");
    assert!(out.contains("widgets"), "stdout={out}");
    assert!(out.trim_end().ends_with(';'), "stdout={out}");
}

#[test]
fn dot_indexes_lists_index_names() {
    let (out, err, code) = run_script(
        None,
        "CREATE TABLE t(a INTEGER, b INTEGER);\n\
         CREATE INDEX t_b_idx ON t(b);\n\
         .indexes t\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("t_b_idx"), "stdout={out}");
}

#[test]
fn dot_databases_reports_main() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("main.db");
    let (out, err, code) = run_script(Some(&path), ".databases\n");
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("main:"), "stdout={out}");
    assert!(out.contains("main.db"), "stdout={out}");
}

#[test]
fn dot_mode_and_headers_apply_to_following_query() {
    let (out, err, code) = run_script(
        None,
        ".mode csv\n\
         .headers on\n\
         CREATE TABLE n(x INTEGER, y TEXT);\n\
         INSERT INTO n VALUES (1, 'a'), (2, 'b');\n\
         SELECT * FROM n ORDER BY x;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("1,a"), "stdout={out}");
    assert!(out.contains("2,b"), "stdout={out}");
}

#[test]
fn dot_nullvalue_and_headers_apply_to_list_output() {
    let (out, err, code) = run_script(
        None,
        ".mode list\n\
         .headers on\n\
         .nullvalue NULL\n\
         SELECT 1 AS a, 'x' AS b, NULL AS c;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("a|b|c"), "stdout={out}");
    assert!(out.contains("1|x|NULL"), "stdout={out}");
}

#[test]
fn cli_flags_select_the_expected_renderers() {
    let cases: &[(&str, &str, &[&str])] = &[
        (
            "-json",
            "SELECT 1 AS a, 'x' AS b, NULL AS c;\n",
            &["\"a\":1", "\"b\":\"x\"", "\"c\":null"],
        ),
        (
            "-line",
            ".nullvalue NULL\nSELECT 1 AS a, 'x' AS b, NULL AS c;\n",
            &["a = 1", "b = x", "c = NULL"],
        ),
        (
            "-column",
            ".headers on\nSELECT 1 AS a, 'x' AS b;\n",
            &["a", "x"],
        ),
        (
            "-box",
            "SELECT 1 AS a, 'x' AS b;\n",
            &["| a | b |", "| 1 | x |"],
        ),
        (
            "-table",
            "SELECT 1 AS a, 'x' AS b;\n",
            &["| a | b |", "| 1 | x |"],
        ),
        (
            "-html",
            ".headers on\nSELECT 1 AS a, '<tag>' AS b;\n",
            &["<TH>a</TH>", "&lt;tag&gt;"],
        ),
        (
            "-ascii",
            "SELECT 1 AS a, 'x' AS b UNION ALL SELECT 2, 'y';\n",
            &["1\u{1f}x\u{1e}2\u{1f}y\u{1e}"],
        ),
    ];

    for &(flag, script, needles) in cases {
        let (out, err, code) = run_script_with_args(&[flag], None, script);
        assert_eq!(code, 0, "flag={flag} stderr={err}");
        for &needle in needles {
            assert!(out.contains(needle), "flag={flag} stdout={out}");
        }
    }
}

#[test]
fn dot_mode_quote_renders_sql_literals() {
    let (out, err, code) = run_script(
        None,
        ".mode quote\n\
         .separator ;\n\
         SELECT 1 AS a, 'x''y' AS b, NULL AS c;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("1,'x''y',NULL"), "stdout={out}");
}

#[test]
fn dot_mode_markdown_renders_pipe_table() {
    let (out, err, code) = run_script(
        None,
        ".mode markdown\n\
         SELECT 1 AS a, 'x' AS b;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("| a | b |"), "stdout={out}");
    assert!(out.contains("| --- | --- |"), "stdout={out}");
}

#[test]
fn dot_print_emits_literal_text() {
    let (out, _err, code) = run_script(None, ".print hello world\n");
    assert_eq!(code, 0);
    assert!(out.contains("hello world"), "stdout={out}");
}

#[test]
fn dot_bail_terminates_on_error() {
    let (_out, _err, code) = run_script(
        None,
        ".bail on\n\
         SELECT * FROM nope;\n\
         .print should_not_print\n",
    );
    assert_eq!(code, 1);
}

#[test]
fn sql_transaction_state_persists_across_input_lines() {
    let (out, err, code) = run_script(
        None,
        ".bail on\n\
         .mode list\n\
         BEGIN;\n\
         CREATE TABLE t(x INT);\n\
         INSERT INTO t VALUES(1);\n\
         COMMIT;\n\
         SELECT count(*) FROM t;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.lines().any(|line| line.trim() == "1"), "stdout={out}");
}

#[test]
fn sql_rollback_state_persists_across_input_lines() {
    let (out, err, code) = run_script(
        None,
        ".bail on\n\
         .mode list\n\
         CREATE TABLE t(x INT);\n\
         BEGIN;\n\
         INSERT INTO t VALUES(1);\n\
         ROLLBACK;\n\
         SELECT count(*) FROM t;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.lines().any(|line| line.trim() == "0"), "stdout={out}");
}

#[test]
fn sql_savepoint_state_persists_across_input_lines() {
    let (out, err, code) = run_script(
        None,
        ".bail on\n\
         .mode list\n\
         CREATE TABLE t(x INT);\n\
         SAVEPOINT s1;\n\
         INSERT INTO t VALUES(1);\n\
         SAVEPOINT s2;\n\
         INSERT INTO t VALUES(2);\n\
         ROLLBACK TO s2;\n\
         RELEASE s2;\n\
         RELEASE s1;\n\
         SELECT group_concat(x,'') FROM t;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.lines().any(|line| line.trim() == "1"), "stdout={out}");
}

#[test]
fn sql_multiline_trigger_body_is_one_statement() {
    let (out, err, code) = run_script(
        None,
        ".bail on\n\
         .mode list\n\
         CREATE TABLE src(x INT);\n\
         CREATE TABLE log(y INT);\n\
         CREATE TRIGGER src_ai AFTER INSERT ON src BEGIN\n\
           INSERT INTO log VALUES (new.x);\n\
         END;\n\
         INSERT INTO src VALUES(7);\n\
         SELECT y FROM log;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.lines().any(|line| line.trim() == "7"), "stdout={out}");
}

#[test]
fn foreign_keys_pragma_persists_across_input_lines() {
    let (_out, err, code) = run_script(
        None,
        ".bail on\n\
         PRAGMA foreign_keys=ON;\n\
         CREATE TABLE p(id INT PRIMARY KEY);\n\
         CREATE TABLE c(pid INT REFERENCES p(id));\n\
         INSERT INTO c VALUES(99);\n",
    );
    assert_eq!(code, 1);
    assert!(
        err.contains("FOREIGN KEY constraint failed"),
        "stderr={err}"
    );
}

#[test]
fn dot_show_dumps_current_settings() {
    let (out, _err, code) = run_script(None, ".show\n");
    assert_eq!(code, 0);
    assert!(out.contains("mode:"), "stdout={out}");
    assert!(out.contains("filename:"), "stdout={out}");
}

#[test]
fn dot_dump_round_trips_through_sqlite3() {
    let Some(version) = sqlite3_version() else {
        eprintln!("sqlite3 binary not found; skipping sqlite3 round-trip test");
        return;
    };
    eprintln!("sqlite3_version={}", version.trim());
    let dir = tempdir().expect("tempdir");
    let dump_path = dir.path().join("dump.sql");

    // Build a small schema and write a `.dump` to disk inside one shell
    // session. Doing both halves in the same process side-steps any WAL /
    // checkpoint timing between separate `redlinedb-cli` invocations, which
    // is unrelated to dot-command correctness.
    let script = format!(
        "CREATE TABLE kv(k INTEGER PRIMARY KEY, v TEXT);\n\
         INSERT INTO kv VALUES (1, 'one');\n\
         INSERT INTO kv VALUES (2, 'two');\n\
         .output {}\n\
         .dump\n\
         .output stdout\n",
        dump_path.display()
    );
    let (_out, err, code) = run_script(None, &script);
    assert_eq!(code, 0, "stderr={err}");
    let dump = std::fs::read_to_string(&dump_path).expect("read dump");
    assert!(dump.contains("BEGIN TRANSACTION;"), "dump={dump}");
    assert!(dump.contains("COMMIT;"), "dump={dump}");
    assert!(dump.contains("CREATE TABLE"), "dump={dump}");
    assert!(dump.contains("INSERT INTO"), "dump={dump}");

    // Pipe the dump through `sqlite3 :memory:` to confirm it round-trips.
    let mut child = Command::new("sqlite3")
        .arg(":memory:")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn sqlite3");
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().expect("stdin");
        stdin.write_all(dump.as_bytes()).expect("write dump");
        stdin
            .write_all(b"SELECT count(*) FROM kv;\n")
            .expect("write select");
    }
    let output = child.wait_with_output().expect("sqlite3 wait");
    assert!(output.status.success(), "stderr={:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains('2'), "sqlite3 stdout={stdout}");
}

#[test]
fn dot_read_executes_file_contents() {
    let dir = tempdir().expect("tempdir");
    let script_path = dir.path().join("setup.sql");
    std::fs::write(
        &script_path,
        "CREATE TABLE imported(id INTEGER);\nINSERT INTO imported VALUES (42);\n",
    )
    .expect("write script");

    let driver = format!(".read {}\nSELECT * FROM imported;\n", script_path.display());
    let (out, err, code) = run_script(None, &driver);
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("42"), "stdout={out}");
}

#[test]
fn unknown_dot_command_reports_error_without_terminating() {
    let (_out, err, code) = run_script(None, ".bogus\n.print after\n");
    assert_eq!(code, 0);
    assert!(err.contains("unknown command"), "stderr={err}");
}

#[test]
fn dot_fullschema_emits_schema_and_sqlite_master_section() {
    let (out, err, code) = run_script(
        None,
        "CREATE TABLE widgets(id INTEGER PRIMARY KEY, name TEXT);\n\
         .fullschema\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    let lower = out.to_ascii_lowercase();
    assert!(lower.contains("create table"), "stdout={out}");
    assert!(out.contains("widgets"), "stdout={out}");
    assert!(
        out.contains("/* sqlite_master */"),
        "fullschema must emit sqlite_master section: stdout={out}"
    );
    assert!(
        out.lines().any(|l| l.starts_with("table|widgets|")),
        "fullschema must dump sqlite_master rows: stdout={out}"
    );
}

#[test]
fn dot_once_redirects_only_the_next_query() {
    let dir = tempdir().expect("tempdir");
    let once_path = dir.path().join("once.txt");
    let script = format!(
        "CREATE TABLE t(x INTEGER);\n\
         INSERT INTO t VALUES (1), (2);\n\
         .once {}\n\
         SELECT x FROM t ORDER BY x;\n\
         SELECT x FROM t ORDER BY x DESC;\n",
        once_path.display()
    );
    let (out, err, code) = run_script(None, &script);
    assert_eq!(code, 0, "stderr={err}");

    let once_contents = std::fs::read_to_string(&once_path).expect("read once file");
    assert!(
        once_contents.contains('1') && once_contents.contains('2'),
        "once file should contain redirected rows: contents={once_contents}"
    );

    let lines: Vec<&str> = out.lines().filter(|l| !l.is_empty()).collect();
    assert!(
        lines.iter().any(|l| l.trim() == "2"),
        "stdout should contain second-query output: stdout={out}"
    );
}

#[test]
fn dot_parameter_set_binds_named_placeholders() {
    let (out, err, code) = run_script(
        None,
        ".parameter set :n 42\n\
         SELECT :n;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(
        out.lines().any(|l| l.trim() == "42"),
        "named parameter should bind: stdout={out}"
    );
}

#[test]
fn dot_parameter_list_and_clear_round_trip() {
    let (out, err, code) = run_script(
        None,
        ".parameter set :a 1\n\
         .parameter set :b two\n\
         .parameter list\n\
         .parameter clear\n\
         .parameter list\n\
         .print done\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    let listed = out.lines().filter(|l| l.contains('\t')).collect::<Vec<_>>();
    assert!(
        listed.iter().any(|l| l.contains(":a") && l.contains('1')),
        "first .parameter list should include :a=1, got: {listed:?}"
    );
    assert!(
        listed.iter().any(|l| l.contains(":b") && l.contains("two")),
        "first .parameter list should include :b=two, got: {listed:?}"
    );
    assert!(out.trim_end().ends_with("done"), "stdout={out}");
}
