//! Validate component directories within one checkout, including source archives.
use crate::{error, Result};
use serde_json::json;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const COMPONENTS: &[(&str, &str)] = &[
    ("redline-core", "."),
    ("redline-testing", "subrepos/redline-testing"),
    ("redline-web", "subrepos/redline-web"),
    ("redline-central", "subrepos/redline-central"),
    ("redline-split-ops", "subrepos/redline-split-ops"),
    ("redline", "subrepos/redline"),
];

pub fn root() -> Option<PathBuf> {
    let start = env::var_os("REDLINE_REPO_ROOT")
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())?;
    start
        .ancestors()
        .find(|p| p.join("subrepos.toml").is_file())
        .map(Path::to_path_buf)
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(error(String::from_utf8_lossy(&output.stderr)));
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn inside(root: &Path, path: &Path) -> Result<PathBuf> {
    let resolved = path.canonicalize()?;
    if !resolved.starts_with(root) {
        return Err(error(format!("path escapes checkout: {}", path.display())));
    }
    Ok(resolved)
}

fn dependencies(root: &Path, base: &Path, value: &toml::Value) -> Result<()> {
    if let Some(table) = value.as_table() {
        if let Some(path) = table.get("path").and_then(toml::Value::as_str) {
            inside(root, &base.join(path))?;
        }
        if table.contains_key("git") {
            return Err(error(
                "Git dependencies are forbidden in the source distribution",
            ));
        }
        for child in table.values() {
            dependencies(root, base, child)?;
        }
    }
    Ok(())
}

fn cargo_paths(root: &Path, dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        if [".git", "target", "node_modules", "dist", ".cache"]
            .iter()
            .any(|n| name == *n)
        {
            continue;
        }
        let path = inside(root, &entry.path())?;
        if entry.file_type()?.is_dir() {
            cargo_paths(root, &path)?;
        } else if name == "Cargo.toml" {
            let value: toml::Value = fs::read_to_string(&path)?.parse()?;
            for key in [
                "dependencies",
                "dev-dependencies",
                "build-dependencies",
                "target",
                "patch",
                "workspace",
            ] {
                if let Some(deps) = value.get(key) {
                    dependencies(root, dir, deps)?;
                }
            }
        }
    }
    Ok(())
}

fn validate(root: &Path, history: bool) -> Result<serde_json::Value> {
    let manifest: toml::Value = fs::read_to_string(root.join("subrepos.toml"))?.parse()?;
    let rows = manifest
        .get("component")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| error("missing components"))?;
    if rows.len() != COMPONENTS.len() {
        return Err(error("expected all six components"));
    }
    for (name, relative) in COMPONENTS {
        let matches: Vec<_> = rows
            .iter()
            .filter(|r| r.get("name").and_then(toml::Value::as_str) == Some(name))
            .collect();
        if matches.len() != 1 {
            return Err(error(format!("missing or duplicate component: {name}")));
        }
        let row = matches[0];
        let field = |key: &str| {
            row.get(key)
                .and_then(toml::Value::as_str)
                .ok_or_else(|| error(format!("missing {key}")))
        };
        if field("path")? != *relative {
            return Err(error(format!("unexpected path for {name}")));
        }
        let path = inside(root, &root.join(relative))?;
        if *relative != "." && path.join(".git").exists() {
            return Err(error("nested Git repositories are forbidden"));
        }
        if !path.join("Cargo.toml").is_file() && *name != "redline" {
            return Err(error(format!("missing workspace: {name}")));
        }
        if history {
            if git(
                root,
                &[
                    "rev-parse",
                    &format!("{}^{{tree}}", field("source_commit")?),
                ],
            )? != field("source_tree")?
            {
                return Err(error("source tree mismatch"));
            }
            if *relative != "."
                && git(
                    root,
                    &[
                        "rev-parse",
                        &format!("{}:{relative}", field("import_commit")?),
                    ],
                )? != field("source_tree")?
            {
                return Err(error("import tree mismatch"));
            }
        }
    }
    cargo_paths(root, &root.join("crates"))?;
    for (_, path) in &COMPONENTS[1..5] {
        cargo_paths(root, &root.join(path))?;
    }
    if history {
        let inventory: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("docs/migration/inventory.json"))?)?;
        for host in inventory["sources"]
            .as_array()
            .ok_or_else(|| error("missing recovery inventory"))?
        {
            for kind in ["repos", "bundles"] {
                for source in host[kind]
                    .as_array()
                    .ok_or_else(|| error("missing recovery sources"))?
                {
                    if let Some(refs) = source["preserved_refs"].as_array() {
                        for pair in refs {
                            let reference = pair[0]
                                .as_str()
                                .ok_or_else(|| error("invalid recovery ref"))?;
                            if Some(git(root, &["rev-parse", "--verify", reference])?.as_str())
                                != pair[1].as_str()
                            {
                                return Err(error(format!("recovery ref mismatch: {reference}")));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(
        json!({"schema":"redline.monorepo-validation/v1","components":COMPONENTS,"history_verified":history}),
    )
}

pub fn run(root: &Path) -> Result<()> {
    let root = root.canonicalize()?;
    let args: Vec<_> = env::args().skip(1).collect();
    match args.first().map(String::as_str).unwrap_or("doctor") {
        "doctor" | "validate" | "control-validate" => {
            if args.len() > 2 || (args.len() == 2 && args[1] != "--history") { return Err(error("validate accepts only --history")); }
            println!("{}", serde_json::to_string_pretty(&validate(&root, args.get(1).is_some())?)?); Ok(())
        }
        "family-ci" | "ci" => {
            validate(&root, false)?;
            let status = Command::new("bash").arg(root.join("scripts/ci-family.sh")).arg("all").current_dir(&root).status()?;
            if !status.success() { return Err(error("family CI failed; no release evidence issued")); } Ok(())
        }
        "--version" | "version" => { println!("redline-proof 0.1.0 (single-checkout)"); Ok(()) }
        _ => Err(error("single-checkout commands: doctor, validate [--history], family-ci; historical consumer promotion is not supported here")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checkout_escape_is_rejected() {
        assert!(inside(&env::temp_dir(), Path::new("/")).is_err());
    }
    #[test]
    fn git_dependency_is_rejected() {
        let deps: toml::Value = "[example]\ngit = \"https://example.test/private\""
            .parse()
            .unwrap();
        assert!(dependencies(Path::new("/"), Path::new("/"), &deps).is_err());
    }
}
