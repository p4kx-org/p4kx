// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use toml::{Table, Value};

use crate::file_size;

const UNSAFE_PACKAGES: &[&str] = &["p4kx-gui", "p4kx-msi-path"];
const UNSAFE_MODULES: &[&str] = &[
    "crates/p4kx-gui/src/dwm.rs",
    "crates/p4kx-msi-path/src/notification_windows.rs",
];
const SAFE_MSI_LIBRARY: &str = "crates/p4kx-msi-path/src/lib.rs";
const SAFE_MSI_FIXTURE: &str = "crates/p4kx-msi-path/examples/msi_registry_fixture.rs";

/// Checks lint inheritance, approved unsafe leaves and unwinding profiles.
pub(crate) fn check(root: &Path) -> Result<Vec<String>> {
    let workspace = read(&root.join("Cargo.toml"))?;
    let lints = workspace
        .get("workspace")
        .and_then(|workspace| workspace.get("lints"))
        .and_then(Value::as_table)
        .context("Cargo.toml has no [workspace.lints]")?;
    let mut violations = Vec::new();
    for (name, manifest) in manifests(root)? {
        let package = read(&manifest)?;
        if let Some(violation) = violation(&name, package.get("lints"), lints) {
            violations.push(violation);
        }
    }
    violations.extend(unsafe_sites(root)?);
    violations.extend(msi_source_contract(root)?);
    violations.extend(aborting_profiles("Cargo.toml", &workspace));
    let config = root.join(".cargo").join("config.toml");
    if config.exists() {
        violations.extend(aborting_profiles(".cargo/config.toml", &read(&config)?));
    }
    Ok(violations)
}

/// Returns one message per profile of `table` with `panic = "abort"`; the Vorbis validation and
/// decoding and the JPEG decoding catch decoder panics, which needs unwinding (ADR-0011, ADR-0013,
/// ADR-0015).
fn aborting_profiles(file: &str, table: &Table) -> Vec<String> {
    table
        .get("profile")
        .and_then(Value::as_table)
        .into_iter()
        .flatten()
        .filter(|(_, profile)| profile.get("panic").and_then(Value::as_str) == Some("abort"))
        .map(|(name, _)| {
            format!("lints: {file} sets panic = \"abort\" in profile {name}; keep unwinding")
        })
        .collect()
}

fn violation(name: &str, lints: Option<&Value>, workspace: &Table) -> Option<String> {
    let (expected, rule) = if UNSAFE_PACKAGES.contains(&name) {
        let mut own = workspace.clone();
        if let Some(rust) = own.get_mut("rust").and_then(Value::as_table_mut) {
            rust.insert("unsafe_code".to_owned(), Value::from("deny"));
        }
        (
            own,
            "must repeat [workspace.lints] with only unsafe_code = \"deny\" changed",
        )
    } else {
        let mut inherit = Table::new();
        inherit.insert("workspace".to_owned(), Value::Boolean(true));
        (
            inherit,
            "must use the workspace lints ([lints] workspace = true)",
        )
    };
    (lints != Some(&Value::Table(expected))).then(|| format!("lints: {name} {rule}"))
}

fn manifests(root: &Path) -> Result<Vec<(String, PathBuf)>> {
    crate::metadata::packages(root)?
        .iter()
        .map(|package| {
            let name = package
                .get("name")
                .and_then(serde_json::Value::as_str)
                .context("package without name")?;
            let manifest = package
                .get("manifest_path")
                .and_then(serde_json::Value::as_str)
                .with_context(|| format!("{name} has no manifest path"))?;
            Ok((name.to_owned(), PathBuf::from(manifest)))
        })
        .collect()
}

fn unsafe_sites(root: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    file_size::collect_rust_files(&root.join("crates"), &mut files)?;
    let mut violations = Vec::new();
    for file in files {
        let shown = file.trim_prefix(root).to_path_buf();
        let text =
            fs::read_to_string(&file).with_context(|| format!("cannot read {}", file.display()))?;
        if names_unsafe_code(&shown, &text) {
            violations.push(format!(
                "lints: {} changes unsafe_code outside the approved leaves",
                shown.display(),
            ));
        }
    }
    Ok(violations)
}

fn names_unsafe_code(relative: &Path, text: &str) -> bool {
    if UNSAFE_MODULES
        .iter()
        .any(|module| relative == Path::new(module))
    {
        return false;
    }
    if [SAFE_MSI_LIBRARY, SAFE_MSI_FIXTURE]
        .iter()
        .any(|root| relative == Path::new(root))
    {
        let Some((before, after)) = text.split_once("#![forbid(unsafe_code)]") else {
            return text.contains("unsafe_code");
        };
        return before.contains("unsafe_code") || after.contains("unsafe_code");
    }
    text.contains("unsafe_code")
}

fn msi_source_contract(root: &Path) -> Result<Vec<String>> {
    let main = fs::read_to_string(root.join("crates/p4kx-msi-path/src/main.rs"))?;
    let library = fs::read_to_string(root.join(SAFE_MSI_LIBRARY))?;
    let fixture = fs::read_to_string(root.join(SAFE_MSI_FIXTURE))?;
    Ok(msi_contract(&main, &library, &fixture))
}

fn msi_contract(main: &str, library: &str, fixture: &str) -> Vec<String> {
    let mut violations = Vec::new();
    if !main
        .replace("\r\n", "\n")
        .contains("#[cfg(windows)]\nmod notification_windows;")
    {
        violations.push("lints: the MSI notification leaf must be cfg(windows)".to_owned());
    }
    for (name, source) in [("library", library), ("native fixture", fixture)] {
        if !source
            .lines()
            .any(|line| line.trim() == "#![forbid(unsafe_code)]")
        {
            violations.push(format!("lints: the MSI {name} must forbid unsafe code"));
        }
    }
    violations
}

fn read(manifest: &Path) -> Result<Table> {
    let text = fs::read_to_string(manifest)
        .with_context(|| format!("cannot read {}", manifest.display()))?;
    text.parse()
        .with_context(|| format!("cannot parse {}", manifest.display()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use toml::{Table, Value};

    use crate::lints;

    type TestResult = Result<(), toml::de::Error>;

    fn workspace() -> Result<Table, toml::de::Error> {
        r#"
            rust = { unsafe_code = "forbid", missing_docs = "deny" }
            clippy = { unwrap_used = "deny" }
        "#
        .parse()
    }

    fn lints(text: &str) -> Result<Value, toml::de::Error> {
        Ok(Value::Table(text.parse()?))
    }

    #[test]
    fn a_package_inheriting_the_workspace_lints_passes() -> TestResult {
        let inherited = lints("workspace = true")?;

        assert_eq!(
            lints::violation("p4kx-archive", Some(&inherited), &workspace()?),
            None,
            "inherits"
        );
        Ok(())
    }

    #[test]
    fn a_package_with_its_own_or_no_lints_fails() -> TestResult {
        let own = lints(r#"rust = { unsafe_code = "deny", missing_docs = "deny" }"#)?;

        assert!(
            lints::violation("p4kx-archive", Some(&own), &workspace()?).is_some(),
            "own table"
        );
        assert!(
            lints::violation("p4kx-archive", None, &workspace()?).is_some(),
            "no table"
        );
        Ok(())
    }

    #[test]
    fn approved_frontends_may_only_lower_unsafe_code_to_deny() -> TestResult {
        let lowered = lints(
            r#"
                rust = { unsafe_code = "deny", missing_docs = "deny" }
                clippy = { unwrap_used = "deny" }
            "#,
        )?;
        let loosened = lints(
            r#"
                rust = { unsafe_code = "deny", missing_docs = "deny" }
                clippy = { unwrap_used = "warn" }
            "#,
        )?;

        for name in ["p4kx-gui", "p4kx-msi-path"] {
            assert_eq!(
                lints::violation(name, Some(&lowered), &workspace()?),
                None,
                "only unsafe_code lowered"
            );
            assert!(
                lints::violation(name, Some(&loosened), &workspace()?).is_some(),
                "another lint loosened"
            );
            assert!(
                lints::violation(name, Some(&lints("workspace = true")?), &workspace()?).is_some(),
                "the approved leaf needs its exact manifest exception"
            );
        }
        Ok(())
    }

    #[test]
    fn a_profile_that_aborts_on_panic_fails() -> TestResult {
        let unwinding: Table = r#"
            [profile.release]
            lto = "thin"
            panic = "unwind"
        "#
        .parse()?;
        let aborting: Table = r#"
            [profile.release]
            lto = "thin"
            [profile.dist]
            inherits = "release"
            panic = "abort"
        "#
        .parse()?;

        assert!(
            lints::aborting_profiles("Cargo.toml", &unwinding).is_empty(),
            "unwinding profiles"
        );
        assert_eq!(
            lints::aborting_profiles("Cargo.toml", &aborting),
            ["lints: Cargo.toml sets panic = \"abort\" in profile dist; keep unwinding"],
            "the aborting profile"
        );
        Ok(())
    }

    #[test]
    fn only_approved_leaves_may_lower_unsafe_code() {
        let expectation = "#![expect(unsafe_code, reason = \"FFI\")]";

        assert!(
            !lints::names_unsafe_code(Path::new("crates/p4kx-gui/src/dwm.rs"), expectation),
            "the DWM module"
        );
        assert!(
            lints::names_unsafe_code(Path::new("crates/p4kx-gui/src/app.rs"), expectation),
            "another module"
        );
        assert!(
            !lints::names_unsafe_code(Path::new("crates/p4kx-gui/src/app.rs"), "fn safe() {}"),
            "no mention"
        );
        assert!(
            !lints::names_unsafe_code(
                Path::new("crates/p4kx-msi-path/src/notification_windows.rs"),
                expectation
            ),
            "the fixed notification leaf is explicitly approved"
        );
        assert!(
            lints::names_unsafe_code(
                Path::new("crates/p4kx-msi-path/src/installer/registry_store.rs"),
                expectation
            ),
            "Registry access has no own unsafe exception"
        );
    }

    #[test]
    fn the_msi_safe_roots_can_only_strengthen_their_inherited_unsafe_lint() {
        for root in [lints::SAFE_MSI_LIBRARY, lints::SAFE_MSI_FIXTURE] {
            let path = Path::new(root);
            assert!(
                !lints::names_unsafe_code(path, "#![forbid(unsafe_code)]"),
                "the safe root explicitly forbids unsafe descendants"
            );
            assert!(
                lints::names_unsafe_code(path, "#![forbid(unsafe_code)]\n#![expect(unsafe_code)]"),
                "a second annotation cannot weaken the safe root"
            );
            assert!(
                lints::names_unsafe_code(path, "#![expect(unsafe_code)]"),
                "the safe root is not an approved native leaf"
            );
        }
    }

    #[test]
    fn the_notification_registration_and_safe_library_contract_are_mandatory() {
        let main = "#[cfg(windows)]\nmod notification_windows;";
        let library = "#![forbid(unsafe_code)]";
        assert!(
            lints::msi_contract(main, library, library).is_empty(),
            "the exact Windows-only leaf leaves the library safe"
        );
        assert_eq!(
            lints::msi_contract("mod notification_windows;", library, library).len(),
            1,
            "unconditional native registration is refused"
        );
        assert_eq!(
            lints::msi_contract(main, "// #![forbid(unsafe_code)]", library).len(),
            1,
            "a commented-out lint cannot satisfy the contract"
        );
        assert_eq!(
            lints::msi_contract(main, library, "// #![forbid(unsafe_code)]").len(),
            1,
            "the test example must independently forbid unsafe code"
        );
    }
}
