use std::{fs, path::Path, process::Command};

fn repository_paths() -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args(["ls-files", "-co", "--exclude-standard", "-z", "--"])
        .output()
        .map_err(|error| format!("failed to execute git ls-files: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("git ls-files emitted non-UTF-8 output: {error}"))?;
    Ok(stdout
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect())
}

fn is_forbidden_source(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "py" | "pyc" | "pyi" | "pyo" | "pyw"
            )
        })
}

fn is_execution_surface(path: &Path, content: &str) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let normalized = path.to_string_lossy().replace('\\', "/");

    matches!(
        extension.as_str(),
        "sh" | "bash" | "zsh" | "ksh" | "yml" | "yaml" | "just" | "mk"
    ) || matches!(
        file_name.as_str(),
        "justfile" | "makefile" | "gnumakefile" | "taskfile" | "package.json"
    ) || file_name.starts_with("dockerfile")
        || normalized == ".gitlab-ci.yml"
        || normalized.starts_with(".github/workflows/")
        || normalized.starts_with("ops/git-hooks/")
        || normalized.starts_with("tools/jankurai-hooks/")
        || content.starts_with("#!")
}

fn normalize_token(raw: &str) -> String {
    raw.trim_matches(|character: char| {
        matches!(
            character,
            '\'' | '"' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | ':' | '\\'
        )
    })
    .rsplit('/')
    .next()
    .unwrap_or_default()
    .to_ascii_lowercase()
}

fn is_forbidden_token(token: &str) -> bool {
    if matches!(
        token,
        "black"
            | "mypy"
            | "nox"
            | "pip"
            | "pip3"
            | "pipenv"
            | "pipx"
            | "poetry"
            | "py3-pip"
            | "pytest"
            | "python-pip"
            | "python3-pip"
            | "ruff"
            | "tox"
            | "uv"
    ) {
        return true;
    }
    if token == "python" || token.starts_with("python:") {
        return true;
    }
    token.strip_prefix("python").is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
    })
}

fn forbidden_invocation(line: &str) -> Option<String> {
    let mut active = line.trim_start();
    if active.is_empty() || (active.starts_with('#') && !active.starts_with("#!")) {
        return None;
    }
    active = active.strip_prefix("#!").unwrap_or(active);

    active
        .split(|character: char| {
            character.is_whitespace() || matches!(character, '|' | '&' | '=' | '<' | '>' | '$')
        })
        .map(normalize_token)
        .find(|token| is_forbidden_token(token))
}

fn policy_violations() -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    for relative in repository_paths()? {
        let path = Path::new(&relative);
        if !path.exists() {
            continue;
        }
        if is_forbidden_source(path) {
            violations.push(format!("{relative}: forbidden source file"));
            continue;
        }
        if !path.is_file() {
            continue;
        }

        let bytes = fs::read(path)
            .map_err(|error| format!("failed to read policy candidate {relative}: {error}"))?;
        let content = match String::from_utf8(bytes) {
            Ok(content) => content,
            Err(error) => {
                if is_execution_surface(path, "") {
                    return Err(format!(
                        "execution surface {relative} is not valid UTF-8: {error}"
                    ));
                }
                continue;
            }
        };
        if !is_execution_surface(path, &content) {
            continue;
        }
        for (index, line) in content.lines().enumerate() {
            if let Some(token) = forbidden_invocation(line) {
                violations.push(format!(
                    "{relative}:{}: forbidden interpreter or package invocation ({token})",
                    index + 1
                ));
            }
        }
    }
    Ok(violations)
}

#[test]
fn repository_has_no_python_files_or_invocations() {
    let violations = policy_violations().unwrap_or_else(|error| panic!("{error}"));
    assert!(
        violations.is_empty(),
        "repository policy violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn invocation_detector_covers_interpreters_packages_and_images() {
    for command in [
        "python tool.py",
        "python3 - <<'PY'",
        "/usr/bin/env python3.12 task.py",
        "pip install example",
        "python3 -m pip install example",
        "apt-get install python3-pip",
        "image: python:3.12",
        "#!/usr/bin/env python3",
        "pytest -q",
        "uv run tool.py",
    ] {
        assert!(
            forbidden_invocation(command).is_some(),
            "missed forbidden command: {command}"
        );
    }
    assert!(forbidden_invocation("# python3 example.py").is_none());
    assert!(forbidden_invocation("printf pipeline-ready").is_none());
}

#[test]
fn source_detector_is_case_insensitive() {
    assert!(is_forbidden_source(Path::new("scripts/tool.py")));
    assert!(is_forbidden_source(Path::new("scripts/tool.PY")));
    assert!(is_forbidden_source(Path::new("scripts/tool.pyw")));
    assert!(is_forbidden_source(Path::new("scripts/tool.pyi")));
    assert!(is_forbidden_source(Path::new("scripts/tool.pyc")));
    assert!(is_forbidden_source(Path::new("scripts/tool.pyo")));
    assert!(!is_forbidden_source(Path::new("scripts/tool.rs")));
}
