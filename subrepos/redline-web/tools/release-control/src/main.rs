use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("release-control: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<String, String> {
    match args.as_slice() {
        [command] if command == "language-boundary" => check_language_boundary(Path::new(".")),
        [command, root] if command == "language-boundary" => {
            check_language_boundary(Path::new(root))
        }
        _ => Err("usage: redline-web-release-control language-boundary [REPO]".to_owned()),
    }
}

fn check_language_boundary(root: &Path) -> Result<String, String> {
    let violations = policy_violations(root)?;
    if violations.is_empty() {
        return Ok("language boundary: no forbidden interpreter files or invocations".to_owned());
    }
    Err(format!(
        "language boundary violations:\n{}",
        violations.join("\n")
    ))
}

fn repository_paths(root: &Path) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("git")
        .args(["-C", root.to_string_lossy().as_ref()])
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
        .map(PathBuf::from)
        .collect())
}

fn forbidden_source(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "py" | "pyc" | "pyi" | "pyo" | "pyw"
            )
        })
}

fn execution_surface(path: &Path, content: &str) -> bool {
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
        || content.starts_with("#!")
}

fn process_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("rs" | "js" | "mjs" | "cjs" | "ts" | "tsx")
    )
}

fn generated_surface(path: &Path) -> bool {
    path.to_string_lossy()
        .replace('\\', "/")
        .starts_with("apps/web/dist/")
}

fn normalize_token(raw: &str) -> String {
    raw.trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '.')
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn forbidden_token(token: &str) -> bool {
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

fn invocation_on_line(path: &Path, line: &str) -> Option<String> {
    let active = line.trim_start();
    if active.is_empty()
        || (active.starts_with('#') && !active.starts_with("#!"))
        || active.starts_with("//")
        || active.starts_with("<!--")
    {
        return None;
    }

    let token = active
        .split(|character: char| {
            character.is_whitespace()
                || matches!(
                    character,
                    '\'' | '"'
                        | '`'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                        | ','
                        | ';'
                        | ':'
                        | '\\'
                        | '|'
                        | '&'
                        | '='
                        | '<'
                        | '>'
                        | '$'
                        | '_'
                )
        })
        .map(normalize_token)
        .find(|token| forbidden_token(token))?;

    if execution_surface(path, active) {
        return Some(token);
    }
    if process_source(path)
        && [
            "Command::new",
            "process::Command",
            "spawn(",
            "spawnSync(",
            "exec(",
            "execFile(",
            "execa(",
        ]
        .iter()
        .any(|marker| active.contains(marker))
    {
        return Some(token);
    }
    None
}

fn policy_violations(root: &Path) -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    for relative in repository_paths(root)? {
        let path = root.join(&relative);
        if !path.exists() {
            continue;
        }
        if forbidden_source(&relative) {
            violations.push(format!("{}: forbidden source file", relative.display()));
            continue;
        }
        if generated_surface(&relative) {
            continue;
        }
        if !path.is_file() {
            continue;
        }

        let bytes = fs::read(&path).map_err(|error| {
            format!(
                "failed to read policy candidate {}: {error}",
                path.display()
            )
        })?;
        let content = match String::from_utf8(bytes) {
            Ok(content) => content,
            Err(error) => {
                if execution_surface(&relative, "") {
                    return Err(format!(
                        "execution surface {} is not valid UTF-8: {error}",
                        relative.display()
                    ));
                }
                continue;
            }
        };
        if !execution_surface(&relative, &content) && !process_source(&relative) {
            continue;
        }
        for (index, line) in content.lines().enumerate() {
            if let Some(token) = invocation_on_line(&relative, line) {
                violations.push(format!(
                    "{}:{}: forbidden interpreter or package invocation ({token})",
                    relative.display(),
                    index + 1
                ));
            }
        }
    }
    Ok(violations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_source_extensions_case_insensitively() {
        for path in ["scripts/tool.py", "scripts/tool.PY", "tool.pyw", "tool.pyi"] {
            assert!(forbidden_source(Path::new(path)), "missed {path}");
        }
        assert!(!forbidden_source(Path::new("tools/release-control.rs")));
    }

    #[test]
    fn detects_direct_and_split_interpreter_commands() {
        let shell = Path::new("ops/ci/fast.sh");
        for line in [
            "python3 tool.py",
            "python_command=\"python\"\"3\"",
            "exec /usr/bin/python3.12 task.py",
            "pip install example",
            "image: python:3.12",
        ] {
            assert!(invocation_on_line(shell, line).is_some(), "missed {line}");
        }
    }

    #[test]
    fn ignores_documentation_comments_and_unrelated_words() {
        let shell = Path::new("ops/ci/fast.sh");
        assert!(invocation_on_line(shell, "# python3 is forbidden here").is_none());
        assert!(invocation_on_line(shell, "printf pipeline-ready").is_none());
    }

    #[test]
    fn repository_satisfies_language_boundary() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let violations = policy_violations(&root).unwrap_or_else(|error| panic!("{error}"));
        assert!(
            violations.is_empty(),
            "repository policy violations:\n{}",
            violations.join("\n")
        );
    }
}
