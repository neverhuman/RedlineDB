use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

const BADGE_JSON: &str = "agent/jankurai-badge.json";
const README: &str = "README.md";
const BEGIN_MARKER: &str = "<!-- jankurai-score-badge:begin -->";
const END_MARKER: &str = "<!-- jankurai-score-badge:end -->";

#[derive(Debug, Deserialize)]
struct Badge {
    message: Option<String>,
    color: Option<String>,
}

pub fn run(repo_root: &Path) -> Result<()> {
    let badge_path = repo_root.join(BADGE_JSON);
    let readme_path = repo_root.join(README);
    let badge: Badge = serde_json::from_str(
        &fs::read_to_string(&badge_path)
            .with_context(|| format!("read {}", badge_path.display()))?,
    )
    .with_context(|| format!("parse {}", badge_path.display()))?;
    let message = badge.message.as_deref().unwrap_or("unknown");
    let color = badge.color.as_deref().unwrap_or("lightgrey");
    let content = fs::read_to_string(&readme_path)
        .with_context(|| format!("read {}", readme_path.display()))?;
    let updated = update_content(&content, message, color)?;
    let changed = updated != content;
    fs::write(&readme_path, updated).with_context(|| format!("write {}", readme_path.display()))?;
    println!(
        "Badge: {message} ({color}) — README {}",
        if changed { "updated" } else { "unchanged" }
    );
    Ok(())
}

fn update_content(content: &str, message: &str, color: &str) -> Result<String> {
    let replacement = badge_block(message, color);
    let mut updated = String::with_capacity(content.len().max(replacement.len()));
    let mut remainder = content;
    let mut replacements = 0;

    while let Some(start) = remainder.find(BEGIN_MARKER) {
        let after_begin = &remainder[start + BEGIN_MARKER.len()..];
        let Some(end) = after_begin.find(END_MARKER) else {
            updated.push_str(remainder);
            remainder = "";
            break;
        };
        updated.push_str(&remainder[..start]);
        updated.push_str(&replacement);
        remainder = &after_begin[end + END_MARKER.len()..];
        replacements += 1;
    }
    updated.push_str(remainder);

    if replacements == 0 {
        bail!("markers not found in {README}");
    }
    Ok(updated)
}

fn badge_block(message: &str, color: &str) -> String {
    let encoded = message.replace('/', "%2F").replace(' ', "%20");
    let badge_url = format!("https://img.shields.io/badge/jankurai-{encoded}-{color}");
    let badge_md = format!("[![Jankurai score: {message}]({badge_url})](agent/repo-score.json)");
    format!("{BEGIN_MARKER}\n{badge_md}\n{END_MARKER}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_exact_marked_block_and_encodes_slash_and_spaces() {
        let input = format!("before\n{BEGIN_MARKER}\nstale\nmultiline\n{END_MARKER}\nafter\n");
        let expected = concat!(
            "before\n",
            "<!-- jankurai-score-badge:begin -->\n",
            "[![Jankurai score: 85/100 advisory](https://img.shields.io/badge/jankurai-85%2F100%20advisory-orange)](agent/repo-score.json)\n",
            "<!-- jankurai-score-badge:end -->\n",
            "after\n"
        );

        assert_eq!(
            update_content(&input, "85/100 advisory", "orange").unwrap(),
            expected
        );
    }

    #[test]
    fn replaces_every_non_greedy_marker_pair() {
        let input =
            format!("{BEGIN_MARKER}old one{END_MARKER}\nmiddle\n{BEGIN_MARKER}old two{END_MARKER}");
        let block = badge_block("unknown", "lightgrey");

        assert_eq!(
            update_content(&input, "unknown", "lightgrey").unwrap(),
            format!("{block}\nmiddle\n{block}")
        );
    }

    #[test]
    fn leaves_an_exact_badge_byte_identical() {
        let content = format!("header\n{}\n", badge_block("38/100 advisory", "red"));

        assert_eq!(
            update_content(&content, "38/100 advisory", "red").unwrap(),
            content
        );
    }

    #[test]
    fn missing_json_fields_use_the_original_defaults() {
        let badge: Badge = serde_json::from_str("{}").unwrap();

        assert_eq!(badge.message.as_deref().unwrap_or("unknown"), "unknown");
        assert_eq!(badge.color.as_deref().unwrap_or("lightgrey"), "lightgrey");
    }

    #[test]
    fn refuses_to_write_without_a_complete_marker_pair() {
        let error = update_content(BEGIN_MARKER, "38/100 advisory", "red").unwrap_err();

        assert_eq!(error.to_string(), "markers not found in README.md");
    }
}
