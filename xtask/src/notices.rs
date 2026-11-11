// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod about_json;
mod markdown;
mod source_notices;

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

const FILE: &str = "THIRD-PARTY-NOTICES.md";
const MARKER: &str = "<!-- GENERATED BELOW THIS LINE -->\n";
/// The `cargo-about` release the generated part is produced with; CI installs the same one.
const CARGO_ABOUT_VERSION: &str = "0.9.2";
const PLACEHOLDERS: [&str; 2] = ["<copyright holders>", "<year>"];

/// Returns a message when the generated part of `THIRD-PARTY-NOTICES.md` is not the current output
/// of `cargo about`.
///
/// # Errors
///
/// When the file cannot be read, has no marker line, or `cargo about` fails.
pub(crate) fn check(root: &Path) -> Result<Vec<String>> {
    let text = read(root)?;
    let generated = generate(root)?;
    stale(&text, &generated)
}

/// Rewrites the generated part of `THIRD-PARTY-NOTICES.md` from `cargo about`.
///
/// # Errors
///
/// When the file cannot be read or written, has no marker line, or `cargo about` fails.
pub(crate) fn write(root: &Path) -> Result<()> {
    let text = read(root)?;
    let generated = generate(root)?;
    let (head, _) = split(&text)?;
    fs::write(root.join(FILE), format!("{head}{MARKER}{generated}"))
        .with_context(|| format!("cannot write {FILE}"))
}

/// Returns a message when the generated part of `text` differs from `generated`.
fn stale(text: &str, generated: &str) -> Result<Vec<String>> {
    Ok(if generated_part(text)? == generated {
        Vec::new()
    } else {
        vec![format!(
            "{FILE} is out of date; run `cargo xtask notices --write` and commit the result"
        )]
    })
}

fn read(root: &Path) -> Result<String> {
    let text =
        fs::read_to_string(root.join(FILE)).with_context(|| format!("cannot read {FILE}"))?;
    Ok(text.replace("\r\n", "\n"))
}

fn split(text: &str) -> Result<(&str, &str)> {
    text.split_once(MARKER)
        .with_context(|| format!("{FILE} has no line `{}`", MARKER.trim_end()))
}

fn generated_part(text: &str) -> Result<&str> {
    split(text).map(|(_, generated)| generated)
}

fn cargo() -> Command {
    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
}

fn install_hint() -> String {
    format!("install it with `cargo install cargo-about --version {CARGO_ABOUT_VERSION} --locked`")
}

fn ensure_version() -> Result<()> {
    let output = cargo()
        .args(["about", "--version"])
        .output()
        .with_context(|| format!("cannot run cargo about; {}", install_hint()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let found = stdout.split_whitespace().last().unwrap_or_default();
    if !output.status.success() || found != CARGO_ABOUT_VERSION {
        bail!(
            "cargo-about {CARGO_ABOUT_VERSION} is required, found `{}`; {}",
            stdout.trim(),
            install_hint()
        );
    }
    Ok(())
}

/// Returns the placeholders of an unfilled SPDX licence template that `generated` still holds.
fn placeholders(generated: &str) -> Vec<&'static str> {
    PLACEHOLDERS
        .into_iter()
        .filter(|placeholder| generated.contains(placeholder))
        .collect()
}

fn generate(root: &Path) -> Result<String> {
    ensure_version()?;
    let target = std::env::temp_dir().join(format!("p4kx-notices-{}.json", std::process::id()));
    let mut command = cargo();
    command
        .args([
            "about",
            "generate",
            "--workspace",
            "--locked",
            "--fail",
            "--format",
            "json",
            "-o",
        ])
        .arg(&target)
        .current_dir(root);
    let contract = fs::read_to_string(root.join(".config/platforms.json"))?;
    for native_target in native_targets(&contract)? {
        command.args(["--target", &native_target]);
    }
    let output = command
        .output()
        .with_context(|| format!("cannot run cargo about; {}", install_hint()));
    let json = output.and_then(|output| {
        if output.status.success() {
            fs::read_to_string(&target).context("cannot read the cargo about output")
        } else {
            bail!(
                "cargo about failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
        }
    });
    let _ignored = fs::remove_file(&target);
    let mut licences = about_json::parse(&json?)?;
    source_notices::apply(&mut licences)?;
    let generated = markdown::render(&licences);
    let unfilled = placeholders(&generated);
    if !unfilled.is_empty() {
        bail!(
            "the generated notices hold an unfilled licence template ({}); add a clarify entry \
             to about.toml that points at the licence file with the real copyright notice",
            unfilled.join(", ")
        );
    }
    Ok(generated)
}

fn native_targets(text: &str) -> Result<Vec<String>> {
    let contract: serde_json::Value = serde_json::from_str(text)?;
    let rows = contract
        .get("targets")
        .and_then(serde_json::Value::as_array)
        .context("platform contract has no target rows")?;
    let targets = rows
        .iter()
        .map(|row| {
            row.get("target")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .context("platform row has no target")
        })
        .collect::<Result<Vec<_>>>()?;
    if targets.is_empty()
        || targets.iter().enumerate().any(|(index, target)| {
            target.is_empty() || targets.iter().take(index).any(|other| other == target)
        })
    {
        bail!("notices require distinct nonempty native targets");
    }
    Ok(targets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_use_every_declared_native_target() {
        let contract = include_str!("../../.config/platforms.json");
        let targets = native_targets(contract).expect("valid native contract");
        assert_eq!(targets.len(), 5, "every required target is admitted");
        assert!(
            targets
                .iter()
                .any(|target| target == "aarch64-apple-darwin"),
            "licence resolution includes the native macOS target"
        );
        assert!(
            targets
                .iter()
                .any(|target| target == "aarch64-unknown-linux-gnu")
        );
        assert!(
            targets
                .iter()
                .any(|target| target == "aarch64-pc-windows-msvc")
        );
        assert!(
            native_targets(r#"{"targets":[{"target":"duplicate"},{"target":"duplicate"}]}"#)
                .is_err()
        );
        assert!(native_targets("{}").is_err());
        assert!(
            native_targets(r#"{"targets":[]}"#).is_err(),
            "empty platform inventory"
        );
    }

    #[test]
    fn generated_part_follows_the_marker() {
        let text = format!("head\n{MARKER}body\n");
        assert_eq!(
            generated_part(&text).ok(),
            Some("body\n"),
            "the text after the marker is the generated part"
        );
    }

    #[test]
    fn current_file_has_no_violation() {
        let text = format!("head\n{MARKER}body\n");
        assert_eq!(
            stale(&text, "body\n").ok(),
            Some(Vec::new()),
            "a file with the generated output is current"
        );
    }

    #[test]
    fn stale_file_is_reported() {
        let text = format!("head\n{MARKER}old body\n");
        let violations = stale(&text, "new body\n").expect("the marker is present");
        assert_eq!(violations.len(), 1, "one message for the stale file");
        assert!(
            violations[0].contains("out of date"),
            "the message names the problem: {violations:?}"
        );
    }

    #[test]
    fn hand_edit_of_the_generated_part_is_reported() {
        let text = format!("head\n{MARKER}body\nextra line\n");
        assert_eq!(
            stale(&text, "body\n").map(|v| v.len()).ok(),
            Some(1),
            "an edit below the marker is overwritten, so it is stale"
        );
    }

    #[test]
    fn unfilled_mit_template_is_reported() {
        assert_eq!(
            placeholders("MIT License\n\nCopyright (c) <year> <copyright holders>\n"),
            ["<copyright holders>", "<year>"],
            "the blank SPDX template is found"
        );
    }

    #[test]
    fn filled_notice_is_accepted() {
        assert!(
            placeholders("Copyright (c) 2018-2021 Emil Ernerfeldt\n").is_empty(),
            "a real copyright line is not a placeholder"
        );
    }

    #[test]
    fn missing_marker_is_an_error() {
        assert!(
            generated_part("head\n").is_err(),
            "a file without the marker is rejected"
        );
    }
}
