//! Per-case output normalizers for Postgres ↔ target shell comparison.
//!
//! Byte-for-byte agreement between two Postgres shells is rare even for the
//! same SQL: psql renders booleans `t`/`f`, blanks NULLs, prints timestamps
//! with microsecond precision, formats arrays with `{...}`, etc. To avoid
//! a fragile per-case "expected_stdout" we run BOTH sides of every compare
//! through this normalizer pipeline, then equality-check the result. Each
//! `Normalizer` is opt-in per case (manifest-declared) and idempotent.
//!
//! The vocabulary is intentionally fixed and small: every new normalizer is
//! an explicit design decision the case author must request.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Normalizer {
    /// Postgres prints booleans as `t` / `f` in unaligned mode; SQLite uses
    /// 1 / 0. This normalizer rewrites isolated `t` / `f` cells to `1` /
    /// `0` so the two sides agree.
    BooleanTfToInt,
    /// In psql `\pset format unaligned` mode an explicit NULL is emitted as
    /// an empty cell unless `\pset null` overrides it. Some setups omit the
    /// `\pset null NULL` directive — this normalizer rewrites blank cells
    /// (between `|` separators, at line start/end) to the token `NULL`.
    PgNullBlankToNull,
    /// Truncate ISO-8601 timestamps to second resolution. `2025-12-31
    /// 23:59:59.123456+00` → `2025-12-31 23:59:59+00`.
    TimestampIsoToSeconds,
    /// Canonicalize numeric strings: strip trailing zeros after the decimal,
    /// rewrite `1e2` as `100`, drop unnecessary leading zeros. Only applied
    /// to cells that parse as f64.
    NumericNormalize,
    /// Reparse each cell as JSON, sort object keys, re-emit in compact form.
    /// Use for JSONB and JSON document columns where Postgres and SQLite
    /// disagree on whitespace / key ordering.
    JsonbCanonical,
    /// Rewrite Postgres array literal `{1,2,3}` to `[1,2,3]` so SQLite's
    /// json_array emission matches.
    PgArrayBraceToBracket,
    /// Strip trailing whitespace from each line.
    StripTrailingWs,
    /// For error-message parity: drop everything after the first line of
    /// stderr (SQLSTATE codes, hint lines, etc.). Apply only to stderr.
    ErrorFirstLineOnly,
    /// Strip JSON-style string quoting from array elements, e.g.
    /// `["a","b","c"]` → `[a,b,c]`. Pairs with PgArrayBraceToBracket so
    /// PG's unquoted-text-array form `{a,b,c}` and RedlineDB's JSON form
    /// `["a","b","c"]` reduce to the same `[a,b,c]` shape. Only applied
    /// to cells that look like a top-level JSON array of strings.
    JsonStringArrayUnquote,
}

/// Apply a single normalizer to text.
pub fn apply(text: &str, normalizer: Normalizer) -> String {
    match normalizer {
        Normalizer::BooleanTfToInt => normalize_boolean_tf(text),
        Normalizer::PgNullBlankToNull => normalize_pg_null_blank(text),
        Normalizer::TimestampIsoToSeconds => normalize_timestamp_seconds(text),
        Normalizer::NumericNormalize => normalize_numeric(text),
        Normalizer::JsonbCanonical => normalize_jsonb_canonical(text),
        Normalizer::PgArrayBraceToBracket => normalize_pg_array(text),
        Normalizer::StripTrailingWs => normalize_strip_trailing_ws(text),
        Normalizer::ErrorFirstLineOnly => normalize_first_line(text),
        Normalizer::JsonStringArrayUnquote => normalize_json_string_array_unquote(text),
    }
}

/// Apply a sequence of normalizers in order; output of N feeds into N+1.
pub fn apply_chain(text: &str, normalizers: &[Normalizer]) -> String {
    let mut current = text.to_owned();
    for n in normalizers {
        current = apply(&current, *n);
    }
    current
}

fn normalize_boolean_tf(text: &str) -> String {
    text.lines()
        .map(|line| {
            line.split('|')
                .map(|cell| match cell.trim() {
                    "t" => "1",
                    "f" => "0",
                    _ => cell,
                })
                .collect::<Vec<&str>>()
                .join("|")
        })
        .collect::<Vec<String>>()
        .join("\n")
        + if text.ends_with('\n') { "\n" } else { "" }
}

fn normalize_pg_null_blank(text: &str) -> String {
    text.lines()
        .map(|line| {
            let cells: Vec<String> = line
                .split('|')
                .map(|cell| {
                    if cell.is_empty() {
                        "NULL".to_owned()
                    } else {
                        cell.to_owned()
                    }
                })
                .collect();
            cells.join("|")
        })
        .collect::<Vec<String>>()
        .join("\n")
        + if text.ends_with('\n') { "\n" } else { "" }
}

fn normalize_timestamp_seconds(text: &str) -> String {
    // ISO-8601 with optional fractional seconds: YYYY-MM-DD HH:MM:SS[.fff][TZ]
    // The naïve approach: find `[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]+` and drop
    // the `.[0-9]+`. We do this with a tiny hand-rolled scanner since adding
    // `regex` to the runner's deps for one normalizer feels heavy.
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == ':' {
            // Look behind two chars to see if we just emitted HH:MM:SS by
            // peeking forward instead — simpler: detect `.<digits>` after
            // `:DD`. We do that by peeking after we collected `[0-9][0-9]`.
        }
        if c == '.' {
            // Greedy fractional-seconds run.
            let mut digit_run = String::new();
            while let Some(&n) = chars.peek() {
                if n.is_ascii_digit() {
                    digit_run.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            // Only strip if the previous character (already emitted) is a
            // digit, indicating we matched `\d\.\d+` — a fractional-seconds
            // pattern, not e.g. `1.5e10` which we DO want to preserve.
            let preceded_by_digit = out
                .chars()
                .rev()
                .nth(1)
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false);
            if !digit_run.is_empty() && preceded_by_digit && out.contains(':') {
                // Strip the `.` we just appended and the digit run.
                out.pop();
            } else {
                out.push_str(&digit_run);
            }
        }
    }
    out
}

fn normalize_numeric(text: &str) -> String {
    text.lines()
        .map(|line| {
            line.split('|')
                .map(|cell| {
                    let trimmed = cell.trim();
                    if trimmed.is_empty() {
                        return cell.to_owned();
                    }
                    if let Ok(v) = trimmed.parse::<f64>() {
                        if v.is_finite() {
                            return format_finite_f64(v);
                        }
                    }
                    cell.to_owned()
                })
                .collect::<Vec<String>>()
                .join("|")
        })
        .collect::<Vec<String>>()
        .join("\n")
        + if text.ends_with('\n') { "\n" } else { "" }
}

fn format_finite_f64(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        // Render as integer when round-trip is lossless.
        format!("{}", v as i64)
    } else {
        // Use the shortest reproducible representation; %.17g is conservative.
        format!("{v}")
    }
}

fn normalize_jsonb_canonical(text: &str) -> String {
    // Best-effort: replace JSON-looking cells with their canonical form. If
    // a cell doesn't parse as JSON we leave it untouched.
    text.lines()
        .map(|line| {
            line.split('|')
                .map(|cell| {
                    let trimmed = cell.trim();
                    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
                        return cell.to_owned();
                    }
                    match serde_json::from_str::<serde_json::Value>(trimmed) {
                        Ok(value) => canonicalize_json(&value),
                        Err(_) => cell.to_owned(),
                    }
                })
                .collect::<Vec<String>>()
                .join("|")
        })
        .collect::<Vec<String>>()
        .join("\n")
        + if text.ends_with('\n') { "\n" } else { "" }
}

fn canonicalize_json(value: &serde_json::Value) -> String {
    let canonical = sort_json_keys(value);
    serde_json::to_string(&canonical).unwrap_or_else(|_| value.to_string())
}

fn sort_json_keys(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut sorted: BTreeMap<&String, serde_json::Value> = BTreeMap::new();
            for (k, v) in map {
                sorted.insert(k, sort_json_keys(v));
            }
            serde_json::Value::Object(sorted.into_iter().map(|(k, v)| (k.clone(), v)).collect())
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(sort_json_keys).collect())
        }
        other => other.clone(),
    }
}

fn normalize_pg_array(text: &str) -> String {
    // Cells that look like `{1,2,3}` become `[1,2,3]`. We don't try to be
    // clever about nested arrays or quoted elements; case authors needing
    // that can pair with JsonbCanonical.
    text.lines()
        .map(|line| {
            line.split('|')
                .map(|cell| {
                    let trimmed = cell.trim();
                    if trimmed.starts_with('{') && trimmed.ends_with('}') {
                        let inner = &trimmed[1..trimmed.len() - 1];
                        format!("[{inner}]")
                    } else {
                        cell.to_owned()
                    }
                })
                .collect::<Vec<String>>()
                .join("|")
        })
        .collect::<Vec<String>>()
        .join("\n")
        + if text.ends_with('\n') { "\n" } else { "" }
}

fn normalize_strip_trailing_ws(text: &str) -> String {
    text.lines()
        .map(|line| line.trim_end().to_owned())
        .collect::<Vec<String>>()
        .join("\n")
        + if text.ends_with('\n') { "\n" } else { "" }
}

fn normalize_first_line(text: &str) -> String {
    text.lines().next().unwrap_or("").to_owned() + if text.ends_with('\n') { "\n" } else { "" }
}

/// `["a","b","c"]` → `[a,b,c]`. The cell is reparsed as a JSON array; if
/// every element is a string, we emit the elements joined by `,` inside
/// brackets. Non-string elements or non-array cells fall through unchanged
/// (so an int-array cell like `[1,2,3]` remains a valid JSON literal).
fn normalize_json_string_array_unquote(text: &str) -> String {
    text.lines()
        .map(|line| {
            line.split('|')
                .map(|cell| {
                    let trimmed = cell.trim();
                    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
                        return cell.to_owned();
                    }
                    let parsed: serde_json::Value = match serde_json::from_str(trimmed) {
                        Ok(v) => v,
                        Err(_) => return cell.to_owned(),
                    };
                    let arr = match parsed {
                        serde_json::Value::Array(a) => a,
                        _ => return cell.to_owned(),
                    };
                    let mut parts: Vec<String> = Vec::with_capacity(arr.len());
                    for v in arr {
                        match v {
                            serde_json::Value::String(s) => parts.push(s),
                            _ => {
                                // Mixed-type — leave the whole cell untouched.
                                return cell.to_owned();
                            }
                        }
                    }
                    format!("[{}]", parts.join(","))
                })
                .collect::<Vec<String>>()
                .join("|")
        })
        .collect::<Vec<String>>()
        .join("\n")
        + if text.ends_with('\n') { "\n" } else { "" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boolean_tf_to_int_rewrites_isolated_cells() {
        let out = apply("a|t|c\nx|f|z\n", Normalizer::BooleanTfToInt);
        assert_eq!(out, "a|1|c\nx|0|z\n");
    }

    #[test]
    fn pg_null_blank_to_null_fills_empty_cells() {
        let out = apply("a||c\n", Normalizer::PgNullBlankToNull);
        assert_eq!(out, "a|NULL|c\n");
    }

    #[test]
    fn timestamp_iso_to_seconds_drops_fractional() {
        let out = apply(
            "2025-01-02 03:04:05.123456+00\n",
            Normalizer::TimestampIsoToSeconds,
        );
        assert_eq!(out, "2025-01-02 03:04:05+00\n");
    }

    #[test]
    fn numeric_normalize_canonicalizes_decimals() {
        let out = apply("1.50|3.14|1e2\n", Normalizer::NumericNormalize);
        assert_eq!(out, "1.5|3.14|100\n");
    }

    #[test]
    fn jsonb_canonical_sorts_keys() {
        let out = apply("{\"b\":1,\"a\":2}|x\n", Normalizer::JsonbCanonical);
        assert_eq!(out, "{\"a\":2,\"b\":1}|x\n");
    }

    #[test]
    fn pg_array_brace_to_bracket() {
        let out = apply("{1,2,3}|other\n", Normalizer::PgArrayBraceToBracket);
        assert_eq!(out, "[1,2,3]|other\n");
    }

    #[test]
    fn strip_trailing_ws() {
        let out = apply("abc   \nxyz\t \n", Normalizer::StripTrailingWs);
        assert_eq!(out, "abc\nxyz\n");
    }

    #[test]
    fn error_first_line_only() {
        let out = apply(
            "ERROR: thing\nHINT: try this\n",
            Normalizer::ErrorFirstLineOnly,
        );
        assert_eq!(out, "ERROR: thing\n");
    }

    #[test]
    fn chain_composes_in_order() {
        let out = apply_chain(
            "1.50|t|\n",
            &[
                Normalizer::PgNullBlankToNull,
                Normalizer::BooleanTfToInt,
                Normalizer::NumericNormalize,
            ],
        );
        assert_eq!(out, "1.5|1|NULL\n");
    }

    #[test]
    fn json_string_array_unquote_strips_quotes() {
        let out = apply(
            "[\"a\",\"b\",\"c\"]|other\n",
            Normalizer::JsonStringArrayUnquote,
        );
        assert_eq!(out, "[a,b,c]|other\n");
    }

    #[test]
    fn json_string_array_unquote_leaves_int_arrays_alone() {
        let out = apply("[1,2,3]\n", Normalizer::JsonStringArrayUnquote);
        assert_eq!(out, "[1,2,3]\n");
    }

    #[test]
    fn json_string_array_unquote_paired_with_brace_to_bracket() {
        // PG side: `{a,b,c}` becomes `[a,b,c]` via PgArrayBraceToBracket;
        // RedlineDB side: `["a","b","c"]` collapses to `[a,b,c]` via the
        // new unquote normalizer.
        let pg = apply("{a,b,c}\n", Normalizer::PgArrayBraceToBracket);
        let rdb = apply("[\"a\",\"b\",\"c\"]\n", Normalizer::JsonStringArrayUnquote);
        assert_eq!(pg, rdb);
    }
}
