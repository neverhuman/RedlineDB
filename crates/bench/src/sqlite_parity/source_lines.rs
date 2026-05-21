use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use walkdir::{DirEntry, WalkDir};

pub(crate) const SQLITE_REFERENCE_LINES: usize = 155_800;

#[derive(Debug, Clone, Copy)]
pub(crate) struct CoreCrate {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) path: &'static str,
    pub(crate) notes: &'static str,
}

pub(crate) const CORE_CRATES: [CoreCrate; 4] = [
    CoreCrate {
        id: "kernel",
        label: "redlinedb-kernel",
        path: "crates/kernel/src",
        notes: "storage, WAL, MVCC, integrity, JSONB, vector indexes",
    },
    CoreCrate {
        id: "sql",
        label: "redlinedb-sql",
        path: "crates/sql/src",
        notes: "parser, planner, executor, JSON1, vectorized execution",
    },
    CoreCrate {
        id: "redlinedb",
        label: "redlinedb",
        path: "crates/redlinedb/src",
        notes: "public Rust facade",
    },
    CoreCrate {
        id: "ffi",
        label: "redlinedb-ffi",
        path: "crates/ffi/src",
        notes: "SQLite-shaped C ABI bridge",
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SourceLineComponent {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) path: String,
    pub(crate) files: usize,
    pub(crate) lines: usize,
    pub(crate) notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SourceLineSummary {
    pub(crate) components: Vec<SourceLineComponent>,
    pub(crate) total_files: usize,
    pub(crate) total_lines: usize,
    pub(crate) sqlite_reference_lines: usize,
}

impl SourceLineSummary {
    pub(crate) fn redlinedb_ksloc(&self) -> f64 {
        ksloc(self.total_lines)
    }

    pub(crate) fn sqlite_reference_ksloc(&self) -> f64 {
        ksloc(self.sqlite_reference_lines)
    }
}

pub(crate) fn scan_core_crates(repo_root: &Path) -> Result<SourceLineSummary> {
    scan_crates(repo_root, &CORE_CRATES)
}

fn scan_crates(repo_root: &Path, crates: &[CoreCrate]) -> Result<SourceLineSummary> {
    let mut components = Vec::new();
    for krate in crates {
        let root = repo_root.join(krate.path);
        if !root.is_dir() {
            bail!(
                "source line scan root is not a directory: {}",
                root.display()
            );
        }
        let mut files = 0usize;
        let mut lines = 0usize;
        for entry in WalkDir::new(&root)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(include_entry)
        {
            let entry = entry.with_context(|| format!("walk {}", root.display()))?;
            if !entry.file_type().is_file() || !is_rust_source(entry.path()) {
                continue;
            }
            files = files.saturating_add(1);
            let text = fs::read_to_string(entry.path())
                .with_context(|| format!("read Rust source {}", entry.path().display()))?;
            lines = lines.saturating_add(count_rust_source_lines(&text));
        }
        components.push(SourceLineComponent {
            id: krate.id.to_owned(),
            label: krate.label.to_owned(),
            path: krate.path.to_owned(),
            files,
            lines,
            notes: krate.notes.to_owned(),
        });
    }
    let total_files = components.iter().map(|component| component.files).sum();
    let total_lines = components.iter().map(|component| component.lines).sum();
    Ok(SourceLineSummary {
        components,
        total_files,
        total_lines,
        sqlite_reference_lines: SQLITE_REFERENCE_LINES,
    })
}

pub(crate) fn count_rust_source_lines(source: &str) -> usize {
    let mut count = 0usize;
    let mut scanner = LineScanner::default();
    let mut pending_attrs = 0usize;
    let mut pending_test_cfg = false;
    let mut skipped_item: Option<SkippedItem> = None;
    for line in source.split_inclusive('\n') {
        let scanned = scanner.scan_line(line);
        if !scanned.has_code {
            continue;
        }
        let trimmed = scanned.code.trim();
        if let Some(skip) = &mut skipped_item {
            skip.observe(trimmed);
            if skip.is_complete() {
                skipped_item = None;
            }
            continue;
        }
        if pending_test_cfg {
            if is_outer_attribute(trimmed) {
                continue;
            }
            let mut skip = SkippedItem::default();
            skip.observe(trimmed);
            if !skip.is_complete() {
                skipped_item = Some(skip);
            }
            pending_test_cfg = false;
            continue;
        }
        if is_test_cfg_attribute(trimmed) {
            pending_attrs = 0;
            pending_test_cfg = true;
            continue;
        }
        if is_outer_attribute(trimmed) {
            pending_attrs = pending_attrs.saturating_add(1);
            continue;
        }
        count = count.saturating_add(pending_attrs).saturating_add(1);
        pending_attrs = 0;
    }
    count
}

pub(crate) fn ksloc(lines: usize) -> f64 {
    lines as f64 / 1_000.0
}

fn include_entry(entry: &DirEntry) -> bool {
    !is_excluded_name(entry.file_name().to_string_lossy().as_ref())
}

fn is_rust_source(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "rs")
        && path
            .file_name()
            .is_some_and(|name| !is_excluded_file_name(name.to_string_lossy().as_ref()))
}

fn is_excluded_name(name: &str) -> bool {
    matches!(name, "tests" | "benches" | "examples")
}

fn is_excluded_file_name(name: &str) -> bool {
    matches!(name, "tests.rs")
}

#[derive(Debug, Clone, Copy)]
enum StringMode {
    Cooked { escaped: bool },
    Raw { hashes: usize },
}

#[derive(Default)]
struct LineScanner {
    block_depth: usize,
    string: Option<StringMode>,
}

struct ScannedLine {
    has_code: bool,
    code: String,
}

impl LineScanner {
    fn scan_line(&mut self, line: &str) -> ScannedLine {
        let mut index = 0usize;
        let bytes = line.as_bytes();
        let mut has_code = self.string.is_some();
        let mut code = String::new();
        while index < bytes.len() {
            if matches!(bytes[index], b'\n' | b'\r') {
                index += 1;
                continue;
            }
            if self.block_depth > 0 {
                if starts_with(bytes, index, b"/*") {
                    self.block_depth = self.block_depth.saturating_add(1);
                    index += 2;
                } else if starts_with(bytes, index, b"*/") {
                    self.block_depth = self.block_depth.saturating_sub(1);
                    index += 2;
                } else {
                    index += 1;
                }
                continue;
            }
            if let Some(mode) = self.string {
                has_code = true;
                match mode {
                    StringMode::Cooked { escaped } => {
                        if escaped {
                            self.string = Some(StringMode::Cooked { escaped: false });
                        } else if bytes[index] == b'\\' {
                            self.string = Some(StringMode::Cooked { escaped: true });
                        } else if bytes[index] == b'"' {
                            self.string = None;
                        }
                        index += 1;
                    }
                    StringMode::Raw { hashes } => {
                        if raw_string_ends_at(bytes, index, hashes) {
                            self.string = None;
                            index += hashes + 1;
                        } else {
                            index += 1;
                        }
                    }
                }
                continue;
            }
            if bytes[index].is_ascii_whitespace() {
                if has_code {
                    code.push(' ');
                }
                index += 1;
            } else if starts_with(bytes, index, b"//") {
                break;
            } else if starts_with(bytes, index, b"/*") {
                self.block_depth = self.block_depth.saturating_add(1);
                index += 2;
            } else {
                has_code = true;
                if let Some((advance, hashes)) = raw_string_starts_at(bytes, index) {
                    code.push('"');
                    self.string = Some(StringMode::Raw { hashes });
                    index += advance;
                } else if let Some(advance) = cooked_string_starts_at(bytes, index) {
                    code.push('"');
                    self.string = Some(StringMode::Cooked { escaped: false });
                    index += advance;
                } else {
                    code.push(bytes[index] as char);
                    index += 1;
                }
            }
        }
        ScannedLine { has_code, code }
    }
}

#[derive(Default)]
struct SkippedItem {
    brace_depth: usize,
    saw_open_brace: bool,
    complete: bool,
}

impl SkippedItem {
    fn observe(&mut self, code: &str) {
        for ch in code.chars() {
            match ch {
                '{' => {
                    self.saw_open_brace = true;
                    self.brace_depth = self.brace_depth.saturating_add(1);
                }
                '}' if self.saw_open_brace => {
                    self.brace_depth = self.brace_depth.saturating_sub(1);
                }
                ';' if !self.saw_open_brace => {
                    self.complete = true;
                }
                _ => {}
            }
        }
        if self.saw_open_brace && self.brace_depth == 0 {
            self.complete = true;
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }
}

fn is_outer_attribute(code: &str) -> bool {
    code.starts_with("#[")
}

fn is_test_cfg_attribute(code: &str) -> bool {
    let compact = code
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect::<String>();
    compact.starts_with("#[cfg(") && !compact.contains("not(test)") && contains_test_token(&compact)
}

fn contains_test_token(code: &str) -> bool {
    code.match_indices("test").any(|(index, _)| {
        let before = code[..index].chars().next_back();
        let after = code[index + "test".len()..].chars().next();
        !before.is_some_and(is_rust_ident_char) && !after.is_some_and(is_rust_ident_char)
    })
}

fn is_rust_ident_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn starts_with(bytes: &[u8], index: usize, pattern: &[u8]) -> bool {
    bytes
        .get(index..index.saturating_add(pattern.len()))
        .is_some_and(|slice| slice == pattern)
}

fn cooked_string_starts_at(bytes: &[u8], index: usize) -> Option<usize> {
    if bytes[index] == b'"' {
        Some(1)
    } else if matches!(bytes[index], b'b' | b'c') && bytes.get(index + 1) == Some(&b'"') {
        Some(2)
    } else {
        None
    }
}

fn raw_string_starts_at(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
    let mut cursor = index;
    if matches!(bytes[cursor], b'b' | b'c') {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'r') {
        return None;
    }
    cursor += 1;
    let hash_start = cursor;
    while bytes.get(cursor) == Some(&b'#') {
        cursor += 1;
    }
    let hashes = cursor.saturating_sub(hash_start);
    if bytes.get(cursor) == Some(&b'"') {
        Some((cursor + 1 - index, hashes))
    } else {
        None
    }
}

fn raw_string_ends_at(bytes: &[u8], index: usize, hashes: usize) -> bool {
    if bytes.get(index) != Some(&b'"') {
        return false;
    }
    (0..hashes).all(|offset| bytes.get(index + 1 + offset) == Some(&b'#'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanner_counts_code_but_not_blank_lines_or_comments() {
        let source = r###"
// file header
fn main() { /* inline comment */ println!("// still code"); }

/*
block comment
*/
let json = r#"{"comment":"/* still code */"}"#;
let text = "unterminated-looking // comment marker";
let value = 1; // trailing comment
/* open */ let after = 2;
"###;

        assert_eq!(count_rust_source_lines(source), 5);
    }

    #[test]
    fn scanner_excludes_inline_cfg_test_items() {
        let source = r###"
#[derive(Clone)]
pub struct Live;

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    fn hidden() {}
}

#[cfg(all(test, feature = "failpoints"))]
fn hidden_fn() {}

#[cfg(test)]
#[path = "tests.rs"]
mod file_tests;

#[inline]
pub fn live() {}

#[cfg(not(test))]
fn prod_only() {}
"###;

        assert_eq!(count_rust_source_lines(source), 6);
    }

    #[test]
    fn scanner_excludes_non_production_folders_and_test_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let crates = [
            CoreCrate {
                id: "core",
                label: "core",
                path: "crates/core/src",
                notes: "fixture",
            },
            CoreCrate {
                id: "api",
                label: "api",
                path: "crates/api/src",
                notes: "fixture",
            },
        ];
        for krate in crates {
            let src = root.join(krate.path);
            fs::create_dir_all(src.join("tests")).expect("create tests");
            fs::create_dir_all(src.join("benches")).expect("create benches");
            fs::create_dir_all(src.join("examples")).expect("create examples");
            fs::write(src.join("lib.rs"), "pub fn live() {}\n").expect("write lib");
            fs::write(src.join("tests.rs"), "pub fn hidden() {}\n").expect("write tests.rs");
            fs::write(src.join("tests").join("unit.rs"), "pub fn hidden() {}\n")
                .expect("write tests dir");
            fs::write(src.join("benches").join("bench.rs"), "pub fn hidden() {}\n")
                .expect("write benches dir");
            fs::write(
                src.join("examples").join("example.rs"),
                "pub fn hidden() {}\n",
            )
            .expect("write examples dir");
        }

        let summary = scan_crates(root, &crates).expect("scan");

        assert_eq!(summary.total_files, 2);
        assert_eq!(summary.total_lines, 2);
        assert_eq!(summary.components[0].lines, 1);
        assert_eq!(summary.components[1].lines, 1);
    }
}
