// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

const INSTALL_ACTION: &str = ".github/actions/install-rust/action.yml";

/// Checks exact tool pins and the workflow installation boundary.
pub(crate) fn check(root: &Path) -> Result<Vec<String>> {
    let toolchain = fs::read_to_string(root.join("rust-toolchain.toml"))?;
    let nextest = fs::read_to_string(root.join(".config/nextest-version"))?;
    let workspace = fs::read_to_string(root.join("Cargo.toml"))?;
    validate_pins(&toolchain, &nextest, &workspace)?;
    let mut violations = Vec::new();
    for directory in [".github/actions", ".github/workflows"] {
        inspect_directory(root, &root.join(directory), &mut violations)?;
    }
    let installer = fs::read_to_string(root.join(INSTALL_ACTION))?;
    for required in [
        "toolchain: ${{ steps.pins.outputs.rust-toolchain }}",
        "tool: nextest@${{ steps.pins.outputs.nextest-version }}",
        "check-build-tools.ps1 -Verify",
    ] {
        if !installer.contains(required) {
            violations.push(format!(
                "{INSTALL_ACTION}: missing pinned tool contract `{required}`"
            ));
        }
    }
    Ok(violations)
}

fn validate_pins(toolchain: &str, nextest: &str, workspace: &str) -> Result<()> {
    let toolchain: toml::Value = toml::from_str(toolchain)?;
    let channel = toolchain
        .get("toolchain")
        .and_then(|value| value.get("channel"))
        .and_then(toml::Value::as_str)
        .context("rust-toolchain.toml has no toolchain.channel")?;
    let compiler = exact_version(channel).context("compiler pin must be MAJOR.MINOR.PATCH")?;
    exact_version(nextest.trim()).context("nextest pin must be MAJOR.MINOR.PATCH")?;
    let workspace: toml::Value = toml::from_str(workspace)?;
    let minimum = workspace
        .get("workspace")
        .and_then(|value| value.get("package"))
        .and_then(|value| value.get("rust-version"))
        .and_then(toml::Value::as_str)
        .context("workspace has no rust-version")?;
    let padded = if minimum.split('.').count() == 2 {
        format!("{minimum}.0")
    } else {
        minimum.to_owned()
    };
    let minimum = exact_version(&padded).context("invalid workspace rust-version")?;
    if compiler < minimum {
        bail!("the pinned compiler is older than the workspace rust-version");
    }
    Ok(())
}

fn exact_version(text: &str) -> Option<[u64; 3]> {
    let mut components = text.split('.');
    let mut version = [0; 3];
    for value in &mut version {
        let component = components.next()?;
        if component.is_empty()
            || !component.bytes().all(|byte| byte.is_ascii_digit())
            || (component.len() > 1 && component.starts_with('0'))
        {
            return None;
        }
        *value = component.parse().ok()?;
    }
    components.next().is_none().then_some(version)
}

fn inspect_directory(root: &Path, directory: &Path, violations: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            inspect_directory(root, &path, violations)?;
        } else if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("yml" | "yaml")
        ) {
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            let text = fs::read_to_string(&path)?;
            violations.extend(workflow_violations(&relative, &text));
        }
    }
    Ok(())
}

fn workflow_violations(path: &str, text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        let installs_compiler = trimmed.contains("uses: dtolnay/rust-toolchain@");
        let installs_nextest = trimmed.starts_with("tool:") && trimmed.contains("nextest");
        let bypasses_pin = trimmed.starts_with("toolchain:")
            && (path != INSTALL_ACTION
                || trimmed != "toolchain: ${{ steps.pins.outputs.rust-toolchain }}");
        let floats_compiler = [
            "cargo +stable",
            "cargo +beta",
            "cargo +nightly",
            "rustup toolchain install stable",
            "rustup toolchain install beta",
            "rustup toolchain install nightly",
        ]
        .iter()
        .any(|command| trimmed.contains(command));
        if bypasses_pin
            || floats_compiler
            || ((installs_compiler || installs_nextest) && path != INSTALL_ACTION)
        {
            violations.push(format!(
                "{path}:{}: install Rust/nextest through the pinned install-rust action",
                index + 1
            ));
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::{exact_version, validate_pins, workflow_violations};

    #[test]
    fn floating_and_partial_versions_are_rejected() {
        for value in [
            "stable",
            "beta",
            "nightly",
            "1.99",
            "1.99.1-beta",
            "01.99.1",
            "1.2.3.4",
        ] {
            assert_eq!(exact_version(value), None, "{value}");
        }
        assert_eq!(exact_version("1.100.0"), Some([1, 100, 0]));
        assert_eq!(exact_version("0.9.151"), Some([0, 9, 151]));
    }

    #[test]
    fn compiler_pin_must_satisfy_workspace_minimum() {
        let workspace = "[workspace.package]\nrust-version = \"1.100.0\"";
        let older = "[toolchain]\nchannel = \"1.99.1\"";
        let current = "[toolchain]\nchannel = \"1.100.0\"";
        assert!(validate_pins(older, "0.9.151", workspace).is_err());
        assert!(validate_pins(current, "0.9.151", workspace).is_ok());
        assert!(validate_pins(current, "latest", workspace).is_err());
        assert!(validate_pins("", "0.9.151", workspace).is_err());
        assert!(validate_pins(current, "0.9.151", "").is_err());
    }

    #[test]
    fn bypassing_the_shared_installer_is_rejected() {
        let source =
            "uses: dtolnay/rust-toolchain@sha\nwith:\n  toolchain: stable\n  tool: nextest";
        assert_eq!(
            workflow_violations(".github/workflows/test.yml", source).len(),
            3
        );
        let pinned = "uses: ./.github/actions/install-rust\nwith:\n  nextest: \"true\"";
        assert_eq!(
            workflow_violations(".github/workflows/test.yml", pinned),
            Vec::<String>::new()
        );
        for selector in [
            "toolchain: \"stable\"",
            "toolchain: 'nightly'",
            "toolchain: 1.99",
        ] {
            assert_eq!(
                workflow_violations(super::INSTALL_ACTION, selector).len(),
                1
            );
        }
    }
}
