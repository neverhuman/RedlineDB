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
fn version_identifies_redlinedb_release_and_sqlite_compatibility() {
    let output = Command::new(cargo_bin("redlinedb-cli"))
        .arg("--version")
        .output()
        .expect("run redlinedb --version");
    assert!(output.status.success(), "stderr={:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(
        stdout.contains(concat!("redlinedb v", env!("CARGO_PKG_VERSION"))),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("SQLite 3.45.1 compatibility"),
        "stdout={stdout}"
    );
    assert_ne!(stdout.trim(), "3.45.1");
}

#[test]
fn memory_mode_query_leaves_working_directory_clean() {
    let dir = tempdir().expect("tempdir");
    let output = Command::new(cargo_bin("redlinedb-cli"))
        .current_dir(dir.path())
        .arg(":memory:")
        .arg("SELECT 1;")
        .output()
        .expect("run redlinedb :memory:");
    assert!(output.status.success(), "stderr={:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert_eq!(stdout.trim(), "1");
    let entries = std::fs::read_dir(dir.path())
        .expect("read tempdir")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect tempdir entries");
    assert!(entries.is_empty(), "memory mode created files: {entries:?}");
}

#[test]
fn batch_bail_memory_mode_is_cwd_clean_and_output_exact() {
    let dir = tempdir().expect("tempdir");
    let mut child = Command::new(cargo_bin("redlinedb-cli"))
        .current_dir(dir.path())
        .arg("--batch")
        .arg("--bail")
        .arg(":memory:")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("run redlinedb --batch --bail :memory:");
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().expect("stdin");
        stdin
            .write_all(
                b"CREATE TABLE t(x INTEGER);\n\
                  INSERT INTO t VALUES (1), (2);\n\
                  SELECT sum(x) FROM t;\n",
            )
            .expect("write stdin");
    }
    let output = child.wait_with_output().expect("wait redlinedb cli");
    assert!(output.status.success(), "stderr={:?}", output.stderr);
    assert_eq!(String::from_utf8(output.stdout).expect("stdout"), "3\n");
    assert_eq!(String::from_utf8(output.stderr).expect("stderr"), "");
    let entries = std::fs::read_dir(dir.path())
        .expect("read tempdir")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect tempdir entries");
    assert!(entries.is_empty(), "memory mode created files: {entries:?}");
}

#[test]
fn deserialize_memory_mode_emits_sqlite_oom_warning_and_continues() {
    let (out, err, code) = run_script_with_args(&["-deserialize", "-list"], None, "SELECT 1;\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "1\n");
    assert_eq!(err, "Error: out of memory\n");
}

#[test]
fn batch_autoincrement_exposes_sqlite_sequence() {
    let (out, err, code) = run_script(
        None,
        "CREATE TABLE t(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL);\n\
         SELECT count(*) FROM sqlite_sequence;\n\
         INSERT INTO t(name) VALUES ('one');\n\
         SELECT name, seq FROM sqlite_sequence;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "0\nt|1\n", "stdout={out}");
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
fn dot_databases_lists_attached_aliases() {
    let dir = tempdir().expect("tempdir");
    let main_path = dir.path().join("main.db");
    let aux_path = dir.path().join("aux.db");
    let script = format!(
        "ATTACH DATABASE '{}' AS aux;\n.databases\n",
        aux_path.display()
    );
    let (out, err, code) = run_script(Some(&main_path), &script);
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("main:"), "stdout={out}");
    assert!(out.contains("aux:"), "stdout={out}");
    assert!(out.contains("main.db"), "stdout={out}");
    assert!(out.contains("aux.db"), "stdout={out}");
}

#[test]
fn dot_list_renders_blob_text_symbolically() {
    let (out, err, code) = run_script(None, ".mode list\nSELECT x'01ab';\n");
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("^A"), "stdout={out}");
    assert!(out.contains('�'), "stdout={out}");
}

#[test]
fn dot_list_renders_zeroblob_as_blank() {
    let (out, err, code) = run_script(None, ".mode list\nSELECT zeroblob(4);\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "\n", "stdout={out}");
}

#[test]
fn dot_crlf_toggles_row_separator() {
    let (out, err, code) = run_script(
        None,
        ".mode list\n\
         .crlf on\n\
         SELECT 1;\n\
         .crlf off\n\
         SELECT 2;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("1\r\n"), "stdout={out:?}");
    assert!(out.contains("2\n"), "stdout={out:?}");
}

#[test]
fn dot_dbinfo_prints_page_size() {
    let (out, err, code) = run_script(None, ".dbinfo\n");
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("database page size"), "stdout={out}");
}

#[test]
fn dot_filectrl_lists_available_controls_for_empty_command() {
    let (out, err, code) = run_script(None, ".filectrl\n");
    assert_eq!(code, 1);
    assert_eq!(err, "");
    assert_eq!(
        out,
        concat!(
            "Available file-controls:\n",
            "  .filectrl chunk_size SIZE\n",
            "  .filectrl data_version \n",
            "  .filectrl has_moved \n",
            "  .filectrl lock_timeout MILLISEC\n",
            "  .filectrl persist_wal [BOOLEAN]\n",
            "  .filectrl psow [BOOLEAN]\n",
            "  .filectrl reserve_bytes [N]\n",
            "  .filectrl size_limit [LIMIT]\n",
            "  .filectrl tempfilename \n",
        )
    );
}

#[test]
fn dot_dbtotxt_prints_pipe_separated_catalog_rows() {
    let (out, err, code) = run_script(
        None,
        "CREATE TABLE t(x INTEGER);\n\
         .dbtotxt\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("|"), "stdout={out}");
}

#[test]
fn dot_recover_emits_dump_sql() {
    let (out, err, code) = run_script(
        None,
        "CREATE TABLE t(x INTEGER);\n\
         INSERT INTO t VALUES (1);\n\
         .recover\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("CREATE TABLE"), "stdout={out}");
    assert!(out.contains("INSERT INTO"), "stdout={out}");
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
            &["a: 1", "b: x", "c: NULL"],
        ),
        (
            "-column",
            ".headers on\nSELECT 1 AS a, 'x' AS b;\n",
            &["a", "x"],
        ),
        (
            "-box",
            "SELECT 1 AS a, 'x' AS b;\n",
            &[
                "\u{2502} a \u{2502} b \u{2502}",
                "\u{2502} 1 \u{2502} x \u{2502}",
            ],
        ),
        (
            "-table",
            "SELECT 1 AS a, 'x' AS b;\n",
            &["| a | b |", "| 1 | x |"],
        ),
        (
            "-html",
            ".headers on\nSELECT 1 AS a, '<tag>' AS b;\n",
            &["<TH>a", "&lt;tag&gt;"],
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
fn delimited_modes_stream_headers_nulls_and_escaping() {
    let (out, err, code) = run_script_with_args(
        &["-csv", "-header"],
        None,
        "SELECT 1 AS a, 'x,y' AS b, NULL AS c UNION ALL SELECT 2, 'quote''d', 'z';\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    // CSV mode uses RFC 4180 row termination (CRLF) and quotes any value
    // containing an apostrophe so it round-trips through the parser.
    assert_eq!(out, "a,b,c\r\n1,\"x,y\",\r\n2,\"quote'd\",z\r\n");

    let (out, err, code) = run_script_with_args(
        &["-tabs"],
        None,
        ".nullvalue NULL\nSELECT 1 AS a, NULL AS b UNION ALL SELECT 2, 'z';\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "1\tNULL\n2\tz\n");
}

#[test]
fn cli_escape_symbol_renders_control_characters_symbolically() {
    let (out, err, code) = run_script_with_args(&["-escape", "symbol"], None, "SELECT char(10);\n");
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains(r"\n"), "stdout={out}");
}

#[test]
fn cli_ifexists_rejects_missing_database() {
    let dir = tempdir().expect("tempdir");
    let missing = dir.path().join("missing.db");
    let output = Command::new(cargo_bin("redlinedb-cli"))
        .arg("-ifexists")
        .arg(&missing)
        .arg("SELECT 1;")
        .output()
        .expect("run redlinedb -ifexists");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("unable to open database file"),
        "stderr={stderr}"
    );
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
    // sqlite3-style markdown separator: dashes are width+2 wide and the
    // pipes are flush against them (no surrounding spaces).
    assert!(out.contains("|---|---|"), "stdout={out}");
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

// ---------------------------------------------------------------------------
// Track E parity: CLI flag-order quirks
// ---------------------------------------------------------------------------

/// Mode flags reset header / nullvalue / separator / newline to the mode
/// defaults, so order matters. `-header -list` puts `-list` *after*
/// `-header`, which resets headers to "off" (list's default).
#[test]
fn mode_flag_after_header_resets_header_to_mode_default() {
    let bin = cargo_bin("redlinedb-cli");
    let out = Command::new(&bin)
        .args(["-header", "-list", "-batch", ":memory:"])
        .arg("SELECT 1 AS a;")
        .output()
        .expect("run cli");
    assert!(out.status.success(), "stderr={:?}", out.stderr);
    // No header row because `-list` ran after `-header`.
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "1\n");

    let out2 = Command::new(&bin)
        .args(["-list", "-header", "-batch", ":memory:"])
        .arg("SELECT 1 AS a;")
        .output()
        .expect("run cli");
    assert!(out2.status.success(), "stderr={:?}", out2.stderr);
    // Header row present because `-header` ran after `-list`.
    assert_eq!(String::from_utf8(out2.stdout).unwrap(), "a\n1\n");
}

#[test]
fn mode_flag_after_nullvalue_resets_nullvalue() {
    let bin = cargo_bin("redlinedb-cli");
    let out = Command::new(&bin)
        .args(["-nullvalue", "NULL", "-list", "-batch", ":memory:"])
        .arg("SELECT NULL, 1;")
        .output()
        .expect("run cli");
    assert!(out.status.success(), "stderr={:?}", out.stderr);
    // `-list` reset nullvalue to default empty string.
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "|1\n");
}

#[test]
fn mode_flag_after_separator_resets_separator() {
    let bin = cargo_bin("redlinedb-cli");
    let out = Command::new(&bin)
        .args(["-separator", ";", "-list", "-batch", ":memory:"])
        .arg("SELECT 1, 2, 3;")
        .output()
        .expect("run cli");
    assert!(out.status.success(), "stderr={:?}", out.stderr);
    // `-list` reset separator to its default `|`.
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "1|2|3\n");
}

#[test]
fn cmd_flag_repeated_runs_each_command() {
    let bin = cargo_bin("redlinedb-cli");
    let out = Command::new(&bin)
        .args([
            "-cmd",
            "SELECT 1;",
            "-cmd",
            "SELECT 2;",
            "-list",
            "-batch",
            ":memory:",
        ])
        .arg("SELECT 3;")
        .output()
        .expect("run cli");
    assert!(out.status.success(), "stderr={:?}", out.stderr);
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "1\n2\n3\n");
}

// ---------------------------------------------------------------------------
// Track E parity: output mode rendering
// ---------------------------------------------------------------------------

#[test]
fn mode_column_centres_headers_and_right_aligns_numerics() {
    let (out, err, code) = run_script(
        None,
        ".mode column\n\
         .headers on\n\
         CREATE TABLE t(a INTEGER, b TEXT, c REAL);\n\
         INSERT INTO t VALUES (1,'abc',3.14),(NULL,'a|b',2.0),(42,'a''b',0.5);\n\
         SELECT * FROM t;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(
        out,
        "a    b    c\n--  ---  ----\n 1  abc  3.14\n    a|b   2.0\n42  a'b   0.5\n"
    );
}

#[test]
fn mode_column_honours_width_and_wraps_long_values() {
    let (out, err, code) = run_script(
        None,
        ".mode column\n\
         .headers on\n\
         .width 5 5 5\n\
         CREATE TABLE t(a INTEGER, b TEXT, c REAL);\n\
         INSERT INTO t VALUES (1,'abc',3.14),(NULL,'longerstring',2.0);\n\
         SELECT * FROM t;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(
        out,
        "  a      b      c\n-----  -----  -----\n    1  abc     3.14\n\n       longe    2.0\n       rstri\n       ng\n"
    );
}

#[test]
fn mode_insert_with_blob_uses_lowercase_x_literal() {
    let (out, err, code) = run_script(
        None,
        ".mode insert t\n\
         CREATE TABLE t(a INTEGER, b BLOB);\n\
         INSERT INTO t VALUES (1, x'01ab');\n\
         SELECT * FROM t;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(
        out.contains("INSERT INTO t VALUES(1,x'01ab');"),
        "stdout={out}"
    );
}

#[test]
fn mode_insert_default_table_name_is_tab() {
    let (out, err, code) = run_script(
        None,
        ".mode insert\nCREATE TABLE x(a); INSERT INTO x VALUES(7);\nSELECT * FROM x;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("INSERT INTO tab VALUES(7);"), "stdout={out}");
}

#[test]
fn mode_box_uses_unicode_line_drawing() {
    let (out, err, code) = run_script(None, ".mode box\nSELECT 1 AS a, 2 AS b;\n");
    assert_eq!(code, 0, "stderr={err}");
    assert!(
        out.starts_with('\u{256d}'),
        "expected box-top, stdout={out}"
    );
    assert!(out.contains('\u{2502}'), "expected box-side, stdout={out}");
}

#[test]
fn mode_table_uses_ascii_pluses_only_around_header() {
    let (out, err, code) = run_script(
        None,
        ".mode table\n\
         SELECT 1 AS a, 'x' AS b UNION ALL SELECT 2, 'y';\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    // Three +---+ border lines: top, after header, bottom (NOT between rows).
    let border_count = out.lines().filter(|l| l.starts_with('+')).count();
    assert_eq!(border_count, 3, "stdout={out}");
}

#[test]
fn mode_html_omits_table_wrapper_and_uses_single_open_tags() {
    let (out, err, code) = run_script(
        None,
        ".mode html\n\
         CREATE TABLE t(a INTEGER); INSERT INTO t VALUES(1);\n\
         SELECT * FROM t;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(!out.contains("<TABLE>"), "stdout={out}");
    assert!(!out.contains("</TH>"), "stdout={out}");
    assert!(!out.contains("</TD>"), "stdout={out}");
    assert!(out.contains("<TR>\n<TH>a"), "stdout={out}");
    assert!(out.contains("<TR>\n<TD>1"), "stdout={out}");
}

#[test]
fn mode_html_renders_null_as_literal_null() {
    let (out, err, code) = run_script(None, ".mode html\nSELECT NULL;\n");
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("<TD>null"), "stdout={out}");
}

#[test]
fn mode_json_emits_one_row_per_line_with_blob_escapes() {
    let (out, err, code) = run_script(
        None,
        ".mode json\n\
         SELECT 1 AS a UNION ALL SELECT 2 UNION ALL SELECT 3;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[{\"a\":1},\n{\"a\":2},\n{\"a\":3}]\n");

    let (out2, err2, code2) = run_script(None, ".mode json\nSELECT x'01ab' AS d;\n");
    assert_eq!(code2, 0, "stderr={err2}");
    assert_eq!(out2, "[{\"d\":\"\\u0001\\u00ab\"}]\n");
}

#[test]
fn mode_csv_quotes_apostrophes_and_uses_crlf_terminators() {
    let (out, err, code) = run_script(None, ".mode csv\nSELECT 'a''b';\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "\"a'b\"\r\n");
}

#[test]
fn mode_markdown_separator_uses_three_dashes_per_cell() {
    let (out, err, code) = run_script(None, ".mode markdown\nSELECT 1 AS a, 2 AS b;\n");
    assert_eq!(code, 0, "stderr={err}");
    // Cell width 1 → separator `---` (3 dashes), wrapped with `|`.
    assert!(out.contains("|---|---|"), "stdout={out}");
}

#[test]
fn mode_markdown_respects_headers_off() {
    let (out, err, code) = run_script(
        None,
        ".mode markdown\n.headers off\nSELECT 1 AS a, 2 AS b;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    // Header + separator suppressed → only the data row remains.
    assert_eq!(out, "| 1 | 2 |\n");
}

#[test]
fn mode_quote_renders_blob_with_lowercase_x_literal() {
    let (out, err, code) = run_script(None, ".mode quote\nSELECT x'4142';\n");
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("x'4142'"), "stdout={out}");
    assert!(!out.contains("X'4142'"), "stdout={out}");
}

#[test]
fn mode_tcl_renders_strings_with_octal_escapes() {
    let (out, err, code) = run_script(
        None,
        ".mode tcl\nCREATE TABLE t(a INTEGER, b TEXT);\nINSERT INTO t VALUES(1,'abc');\nSELECT * FROM t;\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "1 \"abc\"\n");
}

#[test]
fn mode_line_uses_colon_and_aligns_names() {
    let (out, err, code) = run_script(None, ".mode line\nSELECT 1 AS a, 'x' AS bb;\n");
    assert_eq!(code, 0, "stderr={err}");
    // Names right-aligned to the longest name width, separator is `: `.
    assert_eq!(out, " a: 1\nbb: x\n");
}

#[test]
fn mode_line_wraps_multiline_values_with_indent() {
    let (out, err, code) = run_script(None, ".mode line\nSELECT 1 AS a, 'first\nsecond' AS b;\n");
    assert_eq!(code, 0, "stderr={err}");
    // Continuation line is indented to align with the value column.
    assert_eq!(out, "a: 1\nb: first\n   second\n");
}

// ---------------------------------------------------------------------------
// Track E parity: dot commands
// ---------------------------------------------------------------------------

#[test]
fn dot_tables_columns_use_five_space_separator_column_major() {
    let (out, err, code) = run_script(
        None,
        "CREATE TABLE alpha(x);\n\
         CREATE TABLE bravo(y);\n\
         CREATE TABLE charlie(z);\n\
         .tables\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    // Each name padded to its own column width, 5-space inter-column gap.
    assert!(
        out.lines().any(|l| l == "alpha     bravo     charlie"),
        "stdout={out}"
    );
}

#[test]
fn dot_schema_sqlite_master_returns_canonical_schema() {
    let (out, err, code) = run_script(None, ".schema sqlite_master\n");
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("CREATE TABLE sqlite_schema ("), "stdout={out}");
    assert!(out.contains("rootpage integer"), "stdout={out}");
}

#[test]
fn dot_schema_accepts_indent_and_nosys_flags() {
    let (out, err, code) = run_script(None, "CREATE TABLE foo(x INTEGER);\n.schema --indent\n");
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.contains("CREATE TABLE foo(x INTEGER);"), "stdout={out}");
    let (out2, err2, code2) = run_script(None, "CREATE TABLE foo(x INTEGER);\n.schema --nosys\n");
    assert_eq!(code2, 0, "stderr={err2}");
    assert!(
        out2.contains("CREATE TABLE foo(x INTEGER);"),
        "stdout={out2}"
    );
}

#[test]
fn dot_databases_uses_quoted_path_for_memory_db() {
    let (out, err, code) = run_script(None, ".databases\n");
    assert_eq!(code, 0, "stderr={err}");
    // sqlite3 renders the main line as: `main: "" r/w` for `:memory:`.
    assert!(out.contains("main: \"\" r/w"), "stdout={out}");
}

#[test]
fn dot_echo_off_is_still_echoed_when_echo_was_on() {
    let (out, _err, code) = run_script(None, ".mode list\n.echo on\n.echo off\nSELECT 1;\n");
    assert_eq!(code, 0);
    // `.echo off` is echoed (echo was on at that line), SELECT is not.
    assert!(out.contains(".echo off"), "stdout={out}");
    assert!(
        !out.contains("SELECT 1"),
        "SELECT should NOT echo, stdout={out}"
    );
    assert!(out.contains("1"), "stdout={out}");
}

#[test]
fn dot_changes_skips_ddl_and_runs_total_for_dml() {
    let (out, err, code) = run_script(
        None,
        ".changes on\n\
         CREATE TABLE t(x);\n\
         INSERT INTO t VALUES(1),(2),(3);\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    // CREATE counts as 0; INSERT counts as 3.
    assert!(
        out.contains("changes: 0   total_changes: 0"),
        "stdout={out}"
    );
    assert!(
        out.contains("changes: 3   total_changes: 3"),
        "stdout={out}"
    );
}

#[test]
fn dot_lint_fkey_indexes_emits_create_index_per_reference() {
    let (out, err, code) = run_script(
        None,
        "CREATE TABLE p(id INTEGER PRIMARY KEY);\n\
         CREATE TABLE c(p_id INTEGER REFERENCES p(id));\n\
         .lint fkey-indexes\n",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert!(
        out.contains("CREATE INDEX 'c_p_id' ON 'c'('p_id'); --> p(id)"),
        "stdout={out}"
    );
}

#[test]
fn dot_separator_double_quoted_argument_expands_escapes() {
    let (out, err, code) = run_script(None, ".mode list\n.separator \"\\t\"\nSELECT 1,2,3;\n");
    assert_eq!(code, 0, "stderr={err}");
    // `\t` inside double quotes becomes a real tab.
    assert_eq!(out, "1\t2\t3\n");
}

#[test]
fn dot_separator_single_quoted_argument_is_literal() {
    let (out, err, code) = run_script(None, ".mode list\n.separator '\\t'\nSELECT 1,2,3;\n");
    assert_eq!(code, 0, "stderr={err}");
    // `\t` inside single quotes stays as the two-character sequence.
    assert_eq!(out, "1\\t2\\t3\n");
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
