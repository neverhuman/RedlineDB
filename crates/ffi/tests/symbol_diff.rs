//! D6 FFI symbol-diff: compute `(libsqlite3 public symbols) - (RedlineDB
//! exports) - (allowlist)`. The set must be empty.
//!
//! Inputs:
//!   - the bundled `libsqlite3-sys` header resolved through `cargo metadata`
//!     at test runtime. The reference set is computed in memory and never
//!     written as a local parity proof artifact.
//!   - `crates/ffi/tests/symbol_allowlist.toml` — strict-justification
//!     allowlist for libsqlite3 symbols that RedlineDB does NOT export.
//!   - `target/debug/libredlinedb.{dylib,so}` — the just-built cdylib whose
//!     exported `sqlite3_*` symbols are diffed against the reference.
//!
//! If the diff is non-empty, the test prints each missing symbol and the
//! remediation hint: either implement the symbol (preferred) or add an
//! entry to `symbol_allowlist.toml` with a strict justification per the
//! file's header policy.
//!
//! Official SQLite parity evidence is produced only through the pinned
//! `neverhuman/redline-testing` release artifact. This local FFI gate prints
//! diagnostics only.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const ALLOWLIST_PATH: &str = "crates/ffi/tests/symbol_allowlist.toml";

/// Compute the workspace root by walking up from `CARGO_MANIFEST_DIR`. The
/// integration test runs from `crates/ffi/`, so the workspace root is two
/// `parent()` steps away.
fn workspace_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .and_then(Path::parent)
        .expect("workspace root above crates/ffi")
        .to_path_buf()
}

fn load_reference_from_bundled_header(workspace: &Path) -> (PathBuf, BTreeSet<String>) {
    let manifest_path = libsqlite3_sys_manifest_path(workspace);
    let header_path = manifest_path
        .parent()
        .expect("libsqlite3-sys manifest directory")
        .join("sqlite3")
        .join("sqlite3.h");
    let text = std::fs::read_to_string(&header_path).unwrap_or_else(|err| {
        panic!(
            "missing bundled sqlite3.h at {}: {err}",
            header_path.display()
        )
    });
    (header_path, parse_sqlite_api_symbols(&text))
}

fn libsqlite3_sys_manifest_path(workspace: &Path) -> PathBuf {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--locked"])
        .env("RUSTC_WRAPPER", "")
        .current_dir(workspace)
        .output()
        .expect("cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("cargo metadata stdout is UTF-8");
    let marker = "\"name\":\"libsqlite3-sys\"";
    let package_start = stdout
        .find(marker)
        .unwrap_or_else(|| panic!("libsqlite3-sys package missing from cargo metadata"));
    let manifest_marker = "\"manifest_path\":\"";
    let manifest_start = stdout[package_start..]
        .find(manifest_marker)
        .map(|offset| package_start + offset + manifest_marker.len())
        .unwrap_or_else(|| panic!("libsqlite3-sys manifest_path missing from cargo metadata"));
    let manifest_end = stdout[manifest_start..]
        .find('"')
        .map(|offset| manifest_start + offset)
        .expect("libsqlite3-sys manifest_path closing quote");
    PathBuf::from(&stdout[manifest_start..manifest_end])
}

fn parse_sqlite_api_symbols(header: &str) -> BTreeSet<String> {
    let stripped = strip_c_comments(header);
    stripped
        .split(';')
        .filter_map(extract_sqlite_api_symbol)
        .collect()
}

fn strip_c_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' {
            match chars.peek().copied() {
                Some('*') => {
                    chars.next();
                    while let Some(block_ch) = chars.next() {
                        if block_ch == '*' && chars.peek().copied() == Some('/') {
                            chars.next();
                            break;
                        }
                    }
                    out.push(' ');
                    continue;
                }
                Some('/') => {
                    chars.next();
                    for line_ch in chars.by_ref() {
                        if line_ch == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                    continue;
                }
                _ => {}
            }
        }
        out.push(ch);
    }
    out
}

fn extract_sqlite_api_symbol(declaration: &str) -> Option<String> {
    let declaration = declaration.trim();
    if !declaration.starts_with("SQLITE_API") {
        return None;
    }
    if let Some(open_paren) = declaration.find('(') {
        return last_identifier(&declaration[..open_paren])
            .filter(|symbol| symbol.starts_with("sqlite3_"));
    }
    last_identifier(declaration).filter(|symbol| symbol.starts_with("sqlite3_"))
}

fn last_identifier(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut end = bytes.len();
    while end > 0 && !is_ident_byte(bytes[end - 1]) {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && is_ident_byte(bytes[start - 1]) {
        start -= 1;
    }
    if start == end {
        return None;
    }
    Some(text[start..end].to_owned())
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Tiny TOML parser dedicated to the `[[exclude]] symbol = "X"` rows. We
/// intentionally do not pull in a TOML crate: keeping `redlinedb-ffi` zero
/// new dev-dependency-cost preserves the build-speed invariant the jankurai
/// audit rubric tracks.
fn load_allowlist(path: &Path) -> BTreeSet<String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|err| {
        panic!(
            "missing allowlist at {}: {err}\n\
             hint: every excluded libsqlite3 symbol needs a one-line justification",
            path.display()
        )
    });
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("symbol") {
            // form: `symbol = "sqlite3_..."`
            let after_eq = rest.split_once('=').map(|(_, v)| v.trim()).unwrap_or("");
            if let Some(first_q) = after_eq.find('"') {
                let after = &after_eq[first_q + 1..];
                if let Some(end_q) = after.find('"') {
                    let name = after[..end_q].trim();
                    if !name.is_empty() {
                        out.insert(name.to_owned());
                    }
                }
            }
        }
    }
    out
}

fn dylib_extension() -> &'static str {
    if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    }
}

fn find_dylib(workspace: &Path) -> PathBuf {
    let ext = dylib_extension();
    // The integration test is built by `cargo test -p redlinedb-ffi`. The
    // cdylib lives at `target/debug/libredlinedb.<ext>` (or
    // `target/release/libredlinedb.<ext>` if the test was invoked under
    // --release). Prefer release first to match `just fuzz-parity-nightly`.
    for profile in ["release", "debug"] {
        let candidate = workspace
            .join("target")
            .join(profile)
            .join(format!("libredlinedb.{ext}"));
        if candidate.exists() {
            return candidate;
        }
    }
    panic!(
        "libredlinedb.{ext} not found under target/{{debug,release}}; \
         run `cargo build -p redlinedb-ffi` first"
    );
}

fn dylib_exports(dylib: &Path) -> BTreeSet<String> {
    // macOS / Linux: `nm -gU` lists external defined symbols. On Linux we
    // prefer `-D --defined-only` because `-U` is mac-specific (means
    // "undefined only" on GNU binutils). Use `--extern-only --defined-only`
    // on Linux and `nm -gU` on macOS to get the same effective set.
    let output = if cfg!(target_os = "macos") {
        Command::new("nm")
            .arg("-gU")
            .arg(dylib)
            .output()
            .expect("nm -gU on macOS")
    } else {
        Command::new("nm")
            .args(["-D", "--defined-only", "--extern-only"])
            .arg(dylib)
            .output()
            .expect("nm -D --defined-only --extern-only on Linux")
    };
    assert!(
        output.status.success(),
        "nm exited non-zero on {}: {}",
        dylib.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("nm output is UTF-8");

    let mut out = BTreeSet::new();
    for line in stdout.lines() {
        // mach-o lines: `<addr> <type> _sqlite3_...`. ELF lines:
        // `<addr> <type> sqlite3_...`. Use rsplit on whitespace to get the
        // symbol token then strip the leading underscore on macOS.
        let symbol = match line.split_whitespace().last() {
            Some(s) => s,
            None => continue,
        };
        let stripped = symbol.strip_prefix('_').unwrap_or(symbol);
        if stripped.starts_with("sqlite3_") {
            out.insert(stripped.to_owned());
        }
    }
    out
}

/// Excluded from the default `cargo test -p redlinedb-ffi` lane because
/// the gate is designed to FAIL until workstream B1-B5 ship the in-scope
/// `sqlite3_*` symbols. Run via the dedicated `just ffi-symbol-diff`
/// proof lane, which invokes `cargo test ... -- --ignored`.
#[test]
#[ignore = "run via `just ffi-symbol-diff`; fails by design per parity-closure plan DOD"]
fn libsqlite3_public_surface_is_covered_or_allowlisted() {
    let workspace = workspace_root();
    let allowlist_path = workspace.join(ALLOWLIST_PATH);

    let (reference_path, reference) = load_reference_from_bundled_header(&workspace);
    assert!(
        !reference.is_empty(),
        "libsqlite3 reference parsed from {} is empty",
        reference_path.display()
    );

    let allowlist = load_allowlist(&allowlist_path);
    let dylib = find_dylib(&workspace);
    let exports = dylib_exports(&dylib);

    // Set arithmetic: missing = reference - exports - allowlist
    let missing: BTreeSet<String> = reference
        .iter()
        .filter(|s| !exports.contains(s.as_str()) && !allowlist.contains(s.as_str()))
        .cloned()
        .collect();

    if !missing.is_empty() {
        let preview: Vec<&String> = missing.iter().take(20).collect();
        let mut msg = String::from(
            "FFI symbol-diff failed: libsqlite3 public symbols are not exported by\n\
             RedlineDB AND not in crates/ffi/tests/symbol_allowlist.toml.\n\
             Fix: either implement the missing symbol in crates/ffi/src/sqlite3_api/\n\
             (preferred) or add an [[exclude]] row with a strict justification per\n\
             the allowlist file header policy.\n\n",
        );
        msg.push_str(&format!(
            "Total missing: {} (showing first {}):\n",
            missing.len(),
            preview.len()
        ));
        for sym in preview {
            msg.push_str("  - ");
            msg.push_str(sym);
            msg.push('\n');
        }
        msg.push_str(&format!(
            "\nReference: {} ({} symbols)\nExports: {} ({} sqlite3_* symbols)\nAllowlist: {} ({} symbols)\n",
            reference_path
                .strip_prefix(&workspace)
                .unwrap_or(&reference_path)
                .display(),
            reference.len(),
            dylib.strip_prefix(&workspace).unwrap_or(&dylib).display(),
            exports.len(),
            allowlist_path
                .strip_prefix(&workspace)
                .unwrap_or(&allowlist_path)
                .display(),
            allowlist.len(),
        ));
        panic!("{msg}");
    }
}
