use std::{fs, process::Command};

use anyhow::{Context, Result, anyhow, bail};

pub(crate) fn run(args: &[String]) -> Result<()> {
    let [toolchain_path] = args else {
        bail!("toolchain-check: usage: toolchain-check RUST_TOOLCHAIN_TOML");
    };
    let document = fs::read_to_string(toolchain_path)
        .with_context(|| format!("toolchain-check: read {toolchain_path}"))?;
    let expected = parse_channel(&document)?;
    let rustc = program_version("rustc")?;
    let cargo = program_version("cargo")?;
    if rustc != expected || cargo != expected {
        bail!("toolchain-check: expected={expected} rustc={rustc} cargo={cargo}");
    }
    println!("rustc={rustc} cargo={cargo}");
    Ok(())
}

fn parse_channel(document: &str) -> Result<String> {
    let mut channel = None;
    for line in document.lines().map(str::trim) {
        let Some(value) = line.strip_prefix("channel") else {
            continue;
        };
        let value = value
            .trim_start()
            .strip_prefix('=')
            .map(str::trim)
            .ok_or_else(|| anyhow!("toolchain-check: malformed channel assignment"))?;
        let value = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("toolchain-check: channel must be a non-empty string"))?;
        if channel.replace(value.to_owned()).is_some() {
            bail!("toolchain-check: channel is declared more than once");
        }
    }
    channel.ok_or_else(|| anyhow!("toolchain-check: missing channel in rust-toolchain.toml"))
}

fn program_version(program: &str) -> Result<String> {
    let output = Command::new(program)
        .arg("--version")
        .output()
        .with_context(|| format!("toolchain-check: execute {program} --version"))?;
    if !output.status.success() {
        bail!(
            "toolchain-check: {program} --version failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let output = String::from_utf8(output.stdout)
        .with_context(|| format!("toolchain-check: {program} output is not UTF-8"))?;
    parse_program_version(&output)
        .ok_or_else(|| anyhow!("toolchain-check: malformed {program} version output"))
}

fn parse_program_version(output: &str) -> Option<String> {
    output.split_whitespace().nth(1).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pinned_channel_and_program_versions() {
        assert_eq!(
            parse_channel("[toolchain]\nchannel = \"1.95.0\"\n").unwrap(),
            "1.95.0"
        );
        assert_eq!(
            parse_program_version("rustc 1.95.0 (abcdef 2026-01-01)").as_deref(),
            Some("1.95.0")
        );
    }

    #[test]
    fn rejects_missing_duplicate_and_unquoted_channels() {
        assert!(parse_channel("[toolchain]\n").is_err());
        assert!(parse_channel("channel = 1.95.0\n").is_err());
        assert!(
            parse_channel("channel = \"1.95.0\"\nchannel = \"stable\"\n")
                .unwrap_err()
                .to_string()
                .contains("more than once")
        );
    }
}
