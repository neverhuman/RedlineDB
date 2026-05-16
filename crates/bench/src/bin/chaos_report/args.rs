//! Argument parsing for the `chaos_report` binary.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

pub(crate) const DEFAULT_VERSION_ROOT_REL: &str = "target/bench/versioned";
pub(crate) const DEFAULT_SUITE: &str = "dick-head-choas";

#[derive(Debug)]
pub(crate) struct Args {
    pub input: PathBuf,
    pub version_root: PathBuf,
    pub suite: String,
}

pub(crate) fn parse_args(argv: &[OsString]) -> Result<Args, String> {
    let mut input: Option<PathBuf> = None;
    let mut version_root: Option<PathBuf> = None;
    let mut suite: Option<String> = None;
    let mut iter = argv.iter().skip(1);
    while let Some(raw) = iter.next() {
        let arg = raw.to_string_lossy().into_owned();
        let (key, inline_value) = match arg.split_once('=') {
            Some((k, v)) => (k.to_string(), Some(v.to_string())),
            None => (arg, None),
        };
        match key.as_str() {
            "--input" => {
                let val = match inline_value {
                    Some(v) => v,
                    None => match iter.next() {
                        Some(v) => v.to_string_lossy().into_owned(),
                        None => return Err("--input requires a value".to_string()),
                    },
                };
                input = Some(PathBuf::from(val));
            }
            "--version-root" => {
                let val = match inline_value {
                    Some(v) => v,
                    None => match iter.next() {
                        Some(v) => v.to_string_lossy().into_owned(),
                        None => return Err("--version-root requires a value".to_string()),
                    },
                };
                version_root = Some(PathBuf::from(val));
            }
            "--suite" => {
                let val = match inline_value {
                    Some(v) => v,
                    None => match iter.next() {
                        Some(v) => v.to_string_lossy().into_owned(),
                        None => return Err("--suite requires a value".to_string()),
                    },
                };
                suite = Some(val);
            }
            "-h" | "--help" => {
                return Err(usage());
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let input = match input {
        Some(v) => v,
        None => return Err("--input is required".to_string()),
    };
    let version_root = match version_root {
        Some(v) => v,
        None => default_version_root(),
    };
    let suite = match suite {
        Some(v) => v,
        None => DEFAULT_SUITE.to_string(),
    };
    Ok(Args {
        input,
        version_root,
        suite,
    })
}

pub(crate) fn usage() -> String {
    String::from(
        "usage: chaos_report --input <stamp-dir> \
[--version-root <dir>] [--suite <name>]",
    )
}

fn default_version_root() -> PathBuf {
    repo_root().join(DEFAULT_VERSION_ROOT_REL)
}

/// Best-effort repo root. Matches the Python script's
/// `Path(__file__).resolve().parents[2]` semantics by walking up from
/// the binary's manifest dir. Falls back to the current working dir
/// when the layout cannot be detected (e.g. installed binary).
pub(crate) fn repo_root() -> PathBuf {
    if let Ok(out) = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
    {
        if out.status.success() {
            if let Ok(trimmed) = std::str::from_utf8(&out.stdout) {
                let path = PathBuf::from(trimmed.trim_end_matches('\n'));
                if !path.as_os_str().is_empty() {
                    return path;
                }
            }
        }
    }
    match env::current_dir() {
        Ok(dir) => dir,
        Err(_) => PathBuf::from("."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_input_only() {
        let argv: Vec<OsString> = vec!["chaos_report".into(), "--input".into(), "/tmp/foo".into()];
        let args = parse_args(&argv).expect("should parse");
        assert_eq!(args.input, PathBuf::from("/tmp/foo"));
        assert_eq!(args.suite, DEFAULT_SUITE);
    }

    #[test]
    fn parse_inline_equals_form() {
        let argv: Vec<OsString> = vec![
            "chaos_report".into(),
            "--input=/tmp/bar".into(),
            "--suite=custom".into(),
            "--version-root=/tmp/v".into(),
        ];
        let args = parse_args(&argv).expect("should parse inline");
        assert_eq!(args.input, PathBuf::from("/tmp/bar"));
        assert_eq!(args.suite, "custom");
        assert_eq!(args.version_root, PathBuf::from("/tmp/v"));
    }

    #[test]
    fn parse_missing_input_is_error() {
        let argv: Vec<OsString> = vec!["chaos_report".into()];
        let err = parse_args(&argv).expect_err("should error");
        assert!(err.contains("--input"));
    }

    #[test]
    fn parse_unknown_arg_is_error() {
        let argv: Vec<OsString> = vec!["chaos_report".into(), "--bogus".into()];
        let err = parse_args(&argv).expect_err("should error");
        assert!(err.contains("unknown argument"));
    }
}
