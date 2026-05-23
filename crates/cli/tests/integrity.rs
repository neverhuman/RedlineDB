use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn cli_source_has_no_sqlite_parity_replay_fastpath() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf();
    let cli_src = repo.join("crates/cli/src");
    let build_rs = repo.join("crates/cli/build.rs");
    let forbidden_files = [cli_src.join("sqlite_parity_fast.rs"), build_rs.clone()];
    for path in forbidden_files {
        assert!(
            !path.exists(),
            "{} must not exist; parity replay tables are forbidden",
            path.display()
        );
    }

    let mut files = Vec::new();
    collect_rust_files(&cli_src, &mut files);
    if build_rs.exists() {
        files.push(build_rs);
    }
    for path in files {
        let text = fs::read_to_string(&path).expect("read cli source");
        for needle in [
            "sqlite_parity_fast",
            "GENERATED_STDIN_CASES",
            "GENERATED_ARG_CASES",
            "expected_stdout",
            "expected_output_map",
            "finish_fast_output",
            "surface_output",
            "without_engine_startup",
            "without engine startup",
        ] {
            assert!(
                !text.contains(needle),
                "{} contains forbidden parity replay marker `{}`",
                path.display(),
                needle
            );
        }
    }
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read source directory") {
        let path = entry.expect("source entry").path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}
