// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reads the JSON that `cargo about generate --format json` prints.

use anyhow::{Context, Result};
use serde_json::Value;

use super::markdown::{Crate, Licence};

/// Crates the generated part leaves out: `dasp_sample`, `enum-map` and `enum-map-derive` have no
/// licence file and are named by hand in `THIRD-PARTY-NOTICES.md`; `difflib` is linked into no
/// binary (a test-only dependency) and its licence file is an unfilled template.
const LEFT_OUT: [&str; 4] = ["dasp_sample", "difflib", "enum-map", "enum-map-derive"];

/// Returns the licence texts with the crates that use them, in `cargo about` order, without the
/// crates in [`LEFT_OUT`] and without licences that no other crate uses.
///
/// # Errors
///
/// When `json` is not the `cargo about` output or lacks a field of a licence or crate.
pub(super) fn parse(json: &str) -> Result<Vec<Licence>> {
    let root: Value = serde_json::from_str(json).context("cargo about printed no valid JSON")?;
    let licences = root
        .get("licenses")
        .and_then(Value::as_array)
        .context("the cargo about output has no `licenses` list")?;
    let mut parsed = Vec::with_capacity(licences.len());
    for value in licences {
        let licence = licence(value)?;
        if !licence.used_by.is_empty() {
            parsed.push(licence);
        }
    }
    Ok(parsed)
}

fn licence(value: &Value) -> Result<Licence> {
    let used_by = value
        .get("used_by")
        .and_then(Value::as_array)
        .context("a licence has no `used_by` list")?;
    let mut crates = Vec::with_capacity(used_by.len());
    for entry in used_by {
        let krate = krate(entry)?;
        if !LEFT_OUT.contains(&krate.name.as_str()) {
            crates.push(krate);
        }
    }
    Ok(Licence {
        name: string(value, "name")?,
        text: string(value, "text")?.replace("\r\n", "\n"),
        used_by: crates,
    })
}

fn krate(entry: &Value) -> Result<Crate> {
    let krate = entry
        .get("crate")
        .context("a `used_by` entry has no `crate`")?;
    Ok(Crate {
        name: string(krate, "name")?,
        version: string(krate, "version")?,
        licence_source: None,
        repository: krate
            .get("repository")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

fn string(value: &Value, key: &str) -> Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .with_context(|| format!("a JSON object has no string `{key}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{"licenses": [
        {"name": "MIT License", "text": "a\r\nb", "used_by": [
            {"crate": {"name": "alpha", "version": "1.0.0", "repository": "https://example.org/a"}},
            {"crate": {"name": "dasp_sample", "version": "0.11.0", "repository": null}}
        ]},
        {"name": "Apache License 2.0", "text": "c", "used_by": [
            {"crate": {"name": "enum-map", "version": "2.0.0"}}
        ]}
    ]}"#;

    #[test]
    fn left_out_crates_and_emptied_licences_are_dropped() {
        let licences = parse(JSON).expect("valid cargo about output");
        assert_eq!(
            licences.len(),
            1,
            "the Apache group held only a left-out crate"
        );
        let mit = &licences[0];
        assert_eq!(mit.text, "a\nb", "line endings are normalised");
        assert_eq!(mit.used_by.len(), 1, "dasp_sample is left out");
        assert_eq!(mit.used_by[0].name, "alpha");
        assert_eq!(
            mit.used_by[0].repository.as_deref(),
            Some("https://example.org/a")
        );
    }

    #[test]
    fn missing_repository_is_none() {
        let json = r#"{"licenses": [{"name": "L", "text": "t", "used_by": [
            {"crate": {"name": "c", "version": "1.0.0"}}]}]}"#;
        let licences = parse(json).expect("valid cargo about output");
        assert_eq!(licences[0].used_by[0].repository, None);
    }

    #[test]
    fn malformed_output_is_an_error() {
        assert!(parse("not json").is_err(), "invalid JSON");
        assert!(parse("{}").is_err(), "no licenses list");
        assert!(
            parse(r#"{"licenses": [{"name": "L", "text": "t"}]}"#).is_err(),
            "no used_by list"
        );
    }
}
