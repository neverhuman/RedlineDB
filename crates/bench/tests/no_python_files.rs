use std::{fs, path::Path, process::Command};

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("bench crate must be two levels below the workspace root")
}

fn is_executable_surface(path: &str) -> bool {
    path.ends_with(".sh")
        || path == ".gitlab-ci.yml"
        || path.starts_with(".github/workflows/")
        || path.starts_with("ops/git-hooks/")
        || path.starts_with("tools/jankurai-hooks/")
        || path.starts_with("just/")
        || matches!(
            Path::new(path).file_name().and_then(|name| name.to_str()),
            Some("Dockerfile" | "Justfile" | "justfile" | "Makefile")
        )
}

fn prohibited_token(line: &str) -> Option<&str> {
    line.split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .find(|token| {
            matches!(
                token.to_ascii_lowercase().as_str(),
                "python" | "python3" | "pip" | "pip3" | "pipx"
            )
        })
}

#[test]
fn repository_contains_no_python_surfaces() {
    let root = workspace_root();
    let output = Command::new("git")
        .args(["ls-files", "-co", "--exclude-standard"])
        .current_dir(root)
        .output()
        .expect("git must be available to enforce the no-Python policy");

    assert!(
        output.status.success(),
        "git ls-files failed while enforcing the no-Python policy: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout =
        String::from_utf8(output.stdout).expect("git ls-files emitted a non-UTF-8 repository path");
    let python_files = stdout
        .lines()
        .filter(|path| path.ends_with(".py") && root.join(path).is_file())
        .collect::<Vec<_>>();

    assert!(
        python_files.is_empty(),
        "Python files are forbidden in redline-core; move the capability to Rust or the owning external repository:\n{}",
        python_files.join("\n")
    );

    let mut invocations = Vec::new();
    for path in stdout.lines().filter(|path| is_executable_surface(path)) {
        let absolute = root.join(path);
        if !absolute.is_file() {
            continue;
        }
        let contents = fs::read_to_string(&absolute).unwrap_or_else(|error| {
            panic!("read executable surface {}: {error}", absolute.display())
        });
        for (index, line) in contents.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') && !(index == 0 && trimmed.starts_with("#!")) {
                continue;
            }
            if let Some(token) = prohibited_token(line) {
                invocations.push(format!("{}:{} ({token})", path, index + 1));
            }
        }
    }

    assert!(
        invocations.is_empty(),
        "Executable Python interpreter/package invocations are forbidden in redline-core:\n{}",
        invocations.join("\n")
    );
}

#[test]
fn invocation_scanner_uses_token_boundaries() {
    assert_eq!(prohibited_token("python3 -c pass"), Some("python3"));
    assert_eq!(prohibited_token("#!/usr/bin/env python3"), Some("python3"));
    assert_eq!(
        prohibited_token("apt-get install python3-pip"),
        Some("python3")
    );
    assert_eq!(prohibited_token("set -euo pipefail"), None);
}
