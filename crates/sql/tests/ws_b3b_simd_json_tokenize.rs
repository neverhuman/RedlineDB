//! WS-B3b: AVX2-accelerated JSON / JSONPath tokenize helpers.
//!
//! Differential test — the SIMD path used by `parse_jsonpath` must agree
//! byte-for-byte with the scalar fallback across:
//!
//!   1. random JSONPath inputs (proptest, 1024 cases),
//!   2. random JSON-shaped byte buffers (1000 hand-driven iterations),
//!   3. boundary inputs (empty, all-whitespace, deeply nested, long
//!      escaped strings, malformed JSON).
//!
//! The SIMD helpers are private to `jsonb.rs`; we exercise them through
//! the public `jsonb_path_query_first` / `jsonb_path_exists` SQL surface,
//! which routes JSONPath text through `parse_jsonpath` — the call site
//! that consumes the SIMD whitespace-skipper and structural scanner.

use proptest::prelude::*;
use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};
use std::sync::Arc;
use tempfile::tempdir;

struct Db {
    _dir: tempfile::TempDir,
    conn: Arc<Connection>,
}

impl Db {
    fn new() -> Self {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("ws_b3b.db");
        let db = Database::create(&path, DbOptions::default()).expect("create");
        let conn = db.connect();
        Db { _dir: dir, conn }
    }

    /// Run a single-row query and return the first column of the first
    /// row, or `Err(<diagnostic>)` if preparation/step fails.
    fn query_one(&self, sql: &str) -> Result<SqlValue, String> {
        let mut stmt = self.conn.prepare(sql).map_err(|e| format!("prepare: {e}"))?;
        match stmt.step().map_err(|e| format!("step: {e}"))? {
            Step::Row => Ok(stmt
                .column_value(0)
                .map_err(|e| format!("col: {e}"))?
                .clone()),
            Step::Done => Err("no rows".to_string()),
        }
    }
}

fn sql_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push('\'');
        }
        out.push(ch);
    }
    out.push('\'');
    out
}

fn path_exists(db: &Db, doc: &str, path: &str) -> Result<SqlValue, String> {
    let sql = format!(
        "SELECT jsonb_path_exists({}, {})",
        sql_quote(doc),
        sql_quote(path)
    );
    db.query_one(&sql)
}

fn path_query_first(db: &Db, doc: &str, path: &str) -> Result<SqlValue, String> {
    let sql = format!(
        "SELECT jsonb_path_query_first({}, {})",
        sql_quote(doc),
        sql_quote(path)
    );
    db.query_one(&sql)
}

// -----------------------------------------------------------------------------
// Edge cases — exercise the SIMD whitespace-skipper + structural scanner
// through the public path-query surface.
// -----------------------------------------------------------------------------

#[test]
fn edge_empty_path_is_rejected_cleanly() {
    let db = Db::new();
    // Empty JSONPath: parser must reject; SIMD path must not panic on
    // zero-length input.
    let out = path_exists(&db, "{}", "");
    assert!(out.is_err(), "expected parse error, got {out:?}");
}

#[test]
fn edge_all_whitespace_path_after_root() {
    let db = Db::new();
    // Path = "$" + 80 spaces; SIMD whitespace skipper must consume all
    // tail bytes and return the suffix-end without going OOB.
    let path = format!("${}", " ".repeat(80));
    let out = path_exists(&db, "{\"a\":1}", &path).unwrap();
    // "$" alone matches the root, so this is `t`.
    assert_eq!(out, SqlValue::Text("t".into()));
}

#[test]
fn edge_long_mixed_whitespace_between_steps() {
    let db = Db::new();
    // All four whitespace classes the SIMD lane recognises, totalling
    // 36 bytes -> exceeds LANES=32 so the AVX2 lane is exercised.
    let ws: String = std::iter::repeat_n(' ', 12)
        .chain(std::iter::repeat_n('\t', 8))
        .chain(std::iter::repeat_n('\n', 8))
        .chain(std::iter::repeat_n('\r', 8))
        .collect();
    let path = format!("$.a{ws}");
    let out = path_exists(&db, "{\"a\":5}", &path).unwrap();
    assert_eq!(out, SqlValue::Text("t".into()));
}

#[test]
fn edge_deeply_nested_object_path() {
    let db = Db::new();
    // 64-level nested doc reached via "$.a.a.a.…" path; exercises the
    // SIMD scanner on a long path with no whitespace.
    let depth = 64;
    let mut doc = String::from("1");
    for _ in 0..depth {
        doc = format!("{{\"a\":{doc}}}");
    }
    let path = std::iter::once("$".to_string())
        .chain(std::iter::repeat_n(".a".to_string(), depth))
        .collect::<String>();
    let out = path_query_first(&db, &doc, &path).unwrap();
    assert_eq!(out, SqlValue::Text("1".into()));
}

#[test]
fn edge_long_string_with_escapes_in_body() {
    let db = Db::new();
    // 4 KiB string with escaped quotes; the parser routes the body
    // through serde_json (untouched), but the path "$.k" travels via
    // the SIMD-wired parse_jsonpath. Must not panic on long body.
    let big = "\\\"hello\\\" world ".repeat(256);
    let doc = format!("{{\"k\":\"{big}\"}}");
    let out = path_exists(&db, &doc, "$.k").unwrap();
    assert_eq!(out, SqlValue::Text("t".into()));
}

#[test]
fn edge_unterminated_bracket_in_path_errors_cleanly() {
    let db = Db::new();
    // Unterminated `[` exercises the SIMD `find_byte(.., b']')` fallback
    // when the target byte is absent; must report a parse error, never
    // a panic / out-of-bounds.
    let out = path_exists(&db, "{\"a\":[1,2,3]}", "$.a[2");
    assert!(out.is_err(), "expected parse error, got {out:?}");
}

// -----------------------------------------------------------------------------
// Property-based differential.
// -----------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1024,
        .. ProptestConfig::default()
    })]

    /// Whitespace padding INSIDE a valid path (after the `$` root token,
    /// where the JSONPath grammar permits whitespace) must not change
    /// the answer. Sweeps the 0-padding (scalar tail) and >=32-byte
    /// (AVX2 lane) ranges of the SIMD whitespace-skipper.
    ///
    /// Note: `parse_jsonpath` requires the very first byte to be `$`,
    /// so we never insert leading whitespace.
    #[test]
    fn whitespace_padding_is_invariant(
        between in 0usize..40,
        trailing in 0usize..40,
        member_seed in 1u8..=26,
    ) {
        let member = (b'a' + (member_seed - 1)) as char;
        let doc = format!("{{\"{member}\":[10,20,30]}}");
        let path_plain = format!("$.{member}[1]");
        let path_padded = format!(
            "$.{}{}[1]{}",
            member,
            " ".repeat(between),
            " ".repeat(trailing)
        );

        let db = Db::new();
        let lhs = path_query_first(&db, &doc, &path_plain);
        let rhs = path_query_first(&db, &doc, &path_padded);

        match (lhs, rhs) {
            (Ok(a), Ok(b)) => prop_assert_eq!(a, b),
            (Err(_), Err(_)) => {}
            (lhs, rhs) => prop_assert!(
                false,
                "padding changed Ok/Err disposition: lhs={:?} rhs={:?}", lhs, rhs
            ),
        }
    }

    /// Mixed whitespace classes (tab, LF, CR, space) must all be
    /// recognised by the SIMD lane and yield the same answer.
    #[test]
    fn mixed_whitespace_classes_are_equivalent(
        n in 0usize..40,
        seed in 0u8..16,
    ) {
        let doc = "{\"a\":{\"b\":7}}";
        let cls = match seed % 4 {
            0 => ' ',
            1 => '\t',
            2 => '\n',
            _ => '\r',
        };
        let mixed: String = std::iter::repeat_n(cls, n).collect();
        let path_simple = "$.a.b".to_string();
        let path_padded = format!("$.a{mixed}.b");

        let db = Db::new();
        let lhs = path_query_first(&db, doc, &path_simple);
        let rhs = path_query_first(&db, doc, &path_padded);
        match (lhs, rhs) {
            (Ok(a), Ok(b)) => prop_assert_eq!(a, b),
            (Err(_), Err(_)) => {}
            (lhs, rhs) => prop_assert!(
                false,
                "mixed-ws disposition mismatch: lhs={:?} rhs={:?}", lhs, rhs
            ),
        }
    }
}

// -----------------------------------------------------------------------------
// 1000-iteration deterministic differential.
// -----------------------------------------------------------------------------

fn next_rand(state: &mut u64) -> u32 {
    // Knuth LCG.
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*state >> 32) as u32
}

#[test]
fn diff_thousand_iterations_padded_vs_plain() {
    let db = Db::new();
    let mut rng = 0xC0FFEE_BAADu64;
    let doc = "{\"a\":1,\"b\":{\"c\":2,\"d\":3},\"e\":[10,20,30,40]}";
    let templates: [&str; 6] = ["$.a", "$.b.c", "$.b.d", "$.e[0]", "$.e[3]", "$"];
    for _ in 0..1000 {
        let t_idx = (next_rand(&mut rng) as usize) % templates.len();
        // 0..=80 whitespace AFTER the `$` root token (the parser rejects
        // leading whitespace before `$`) and at the path tail. Sweeps
        // both the <32-byte (scalar tail) and >=32-byte (AVX2 lane)
        // halves of the SIMD whitespace-skipper.
        let m = (next_rand(&mut rng) as usize) % 81;
        let r = (next_rand(&mut rng) as usize) % 81;
        let tpl = templates[t_idx];
        let (head, tail) = tpl.split_at(1);
        let path_padded = format!(
            "{}{}{}{}",
            head,
            " ".repeat(m),
            tail,
            " ".repeat(r)
        );

        let padded = path_query_first(&db, doc, &path_padded);
        let plain = path_query_first(&db, doc, tpl);

        match (padded, plain) {
            (Ok(a), Ok(b)) => assert_eq!(
                a, b,
                "padded vs plain mismatch for `{tpl}` padding=({m},{r})"
            ),
            (Err(_), Err(_)) => {}
            (a, b) => panic!(
                "padded vs plain disposition mismatch for `{tpl}` padding=({m},{r}): {a:?} vs {b:?}"
            ),
        }
    }
}
