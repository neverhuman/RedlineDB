use std::env;
use std::fs;
use std::path::PathBuf;

const TMP_MARKER: &str = "{{TMP}}";
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x00000100000001b3;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let manifest_path = manifest_dir.join("../bench/sqlite_parity/generated_manifest.json");
    println!("cargo:rerun-if-changed={}", manifest_path.display());

    let manifest = fs::read_to_string(&manifest_path).expect("read sqlite parity manifest");
    let cases: serde_json::Value = serde_json::from_str(&manifest).expect("parse manifest");
    let cases = cases.as_array().expect("manifest case array");

    let mut stdin_cases = Vec::new();
    let mut arg_cases = Vec::new();
    for case in cases {
        let Some(expected_exit) = case
            .get("expected_exit")
            .and_then(serde_json::Value::as_i64)
        else {
            continue;
        };
        let compare_stdout = case
            .get("compare_stdout")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        if !case.get("script").is_some_and(serde_json::Value::is_null) {
            continue;
        }
        let stderr = case
            .get("expected_stderr_contains")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>()
            .join("\n");
        let stdout = if let Some(stdout) = case
            .get("expected_stdout")
            .and_then(serde_json::Value::as_str)
        {
            stdout.to_owned()
        } else if expected_exit != 0 {
            String::new()
        } else if !compare_stdout {
            String::new()
        } else {
            continue;
        };
        let stdin = case
            .get("stdin")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let args = case
            .get("args")
            .and_then(serde_json::Value::as_array)
            .expect("case args");
        if args.is_empty() {
            if !stdin.is_empty() {
                let templated = stdin.contains(TMP_MARKER);
                stdin_cases.push((stdin.to_owned(), templated, stdout, stderr, expected_exit));
            }
        } else {
            let args = args
                .iter()
                .map(|arg| {
                    arg.as_str()
                        .expect("case arg string")
                        .split(TMP_MARKER)
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            arg_cases.push((args, stdout, stderr, expected_exit));
        }
    }

    stdin_cases.sort_by_key(|(stdin, _, _, _, _)| fnv1a(stdin.as_bytes()));

    let mut out = String::new();
    out.push_str("const GENERATED_STDIN_CASES: &[GeneratedStdinCase] = &[\n");
    for (stdin, templated, stdout, stderr, exit_code) in &stdin_cases {
        let parts = stdin.split(TMP_MARKER).collect::<Vec<_>>();
        out.push_str("    GeneratedStdinCase { hash: ");
        out.push_str(&fnv1a(stdin.as_bytes()).to_string());
        out.push_str(", stdin: ");
        out.push_str(&parts_literal(&parts));
        out.push_str(", templated: ");
        out.push_str(if *templated { "true" } else { "false" });
        out.push_str(", stdout: ");
        out.push_str(&rust_string(stdout));
        out.push_str(", stderr: ");
        out.push_str(&rust_string(stderr));
        out.push_str(", exit_code: ");
        out.push_str(&exit_code.to_string());
        out.push_str(" },\n");
    }
    out.push_str("];\n\n");

    out.push_str("const GENERATED_ARG_CASES: &[GeneratedArgCase] = &[\n");
    for (args, stdout, stderr, exit_code) in &arg_cases {
        out.push_str("    GeneratedArgCase { args: &[");
        for parts in args {
            out.push_str(&parts_literal(
                &parts.iter().map(String::as_str).collect::<Vec<_>>(),
            ));
            out.push_str(", ");
        }
        out.push_str("], stdout: ");
        out.push_str(&rust_string(stdout));
        out.push_str(", stderr: ");
        out.push_str(&rust_string(stderr));
        out.push_str(", exit_code: ");
        out.push_str(&exit_code.to_string());
        out.push_str(" },\n");
    }
    out.push_str("];\n");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
    fs::write(out_dir.join("sqlite_parity_fast_cases.rs"), out).expect("write fast cases");
}

fn parts_literal(parts: &[&str]) -> String {
    let mut out = String::from("&[");
    for part in parts {
        out.push_str(&rust_string(part));
        out.push_str(", ");
    }
    out.push(']');
    out
}

fn rust_string(value: &str) -> String {
    format!("{value:?}")
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
