// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::catalog_support::{EXPORTED_AT, write_archive};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn catalog_defaults_to_the_versioned_marker_document() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = write_archive(directory.path())?;
    let marker = catalog(&archive, &directory.path().join("markers.json"), &[])?;
    let explicit = catalog(
        &archive,
        &directory.path().join("explicit-markers.json"),
        &["--provenance", "markers-only"],
    )?;
    assert_eq!(marker, explicit, "explicit and default marker bytes");
    let full = catalog(
        &archive,
        &directory.path().join("full.json"),
        &["--provenance", "full"],
    )?;
    let marker: Value = serde_json::from_slice(&marker)?;
    let full: Value = serde_json::from_slice(&full)?;
    verify_shapes(&marker, &full)?;
    assert_manufacturer(&marker, &full)?;
    Ok(())
}

#[test]
fn schema_json_is_compact_and_independent_of_archive_configuration() -> TestResult {
    let pretty = schema_output(&[])?;
    let compact = schema_output(&["--json"])?;
    assert!(pretty.lines().count() > 1, "bare schema stays indented");
    assert_eq!(compact.lines().count(), 1, "JSON schema is a single line");
    let pretty: Value = serde_json::from_str(&pretty)?;
    let compact: Value = serde_json::from_str(&compact)?;
    assert!(pretty == compact, "formatting keeps the same schema");
    Ok(())
}

fn verify_shapes(marker: &Value, full: &Value) -> TestResult {
    assert_eq!(marker["format"], "p4kx-catalog-markers", "marker format");
    assert_eq!(marker["formatVersion"], 1, "marker format version");
    assert_eq!(
        marker
            .pointer("/catalog/schemaVersion")
            .and_then(Value::as_u64),
        Some(3),
        "catalog data version"
    );
    assert!(
        marker.get("schemaVersion").is_none(),
        "distinct top-level format"
    );
    assert!(
        marker["catalog"].get("provenance").is_none(),
        "marker catalogue omits inventory"
    );
    assert_eq!(full["schemaVersion"], 3, "standalone v3 shape");
    assert!(
        full.get("format").is_none(),
        "full output uses the standalone shape"
    );
    assert!(
        full.get("provenance").is_some(),
        "full provenance inventory"
    );

    let marker_schema = schema(&[])?;
    let full_schema = schema(&["--provenance", "full"])?;
    let marker_validator = jsonschema::validator_for(&marker_schema)?;
    let full_validator = jsonschema::validator_for(&full_schema)?;
    assert!(
        marker_validator.is_valid(marker),
        "marker schema: {:?}",
        marker_validator
            .iter_errors(marker)
            .take(12)
            .map(|error| error.to_string())
            .collect::<Vec<_>>(),
    );
    assert!(
        full_validator.is_valid(full),
        "full schema: {:?}",
        full_validator
            .iter_errors(full)
            .take(12)
            .map(|error| error.to_string())
            .collect::<Vec<_>>(),
    );
    assert!(
        !marker_validator.is_valid(full),
        "marker schema rejects full"
    );
    assert!(
        !full_validator.is_valid(marker),
        "full schema rejects markers"
    );
    Ok(())
}

fn assert_manufacturer(marker: &Value, full: &Value) -> TestResult {
    let pointer = "/definitions/manufacturers/rows/0/localization";
    let original = full.pointer(pointer).ok_or("full manufacturer")?;
    let compact = marker["catalog"]
        .pointer(pointer)
        .ok_or("marker manufacturer")?;
    for field in ["state", "containsEncryptedSource"] {
        assert_eq!(original[field], compact[field], "same {field}");
    }
    let nested = "/value/name/value";
    assert_eq!(
        original.pointer(nested),
        compact.pointer(nested),
        "canonical name reference"
    );
    assert!(original.get("sourceUsesRefs").is_some(), "full references");
    assert!(
        compact.get("sourceUsesRefs").is_none(),
        "marker omits references"
    );
    assert!(
        compact.get("sourceSetComplete").is_none(),
        "marker omits proof completeness"
    );
    assert_eq!(
        compact.as_object().map(serde_json::Map::len),
        Some(3),
        "marker cell has the three published keys",
    );
    Ok(())
}

fn catalog(archive: &Path, out: &Path, options: &[&str]) -> Result<Vec<u8>, Box<dyn Error>> {
    let result = p4kx_testkit::p4kx()?
        .arg("catalog")
        .args(options)
        .arg(archive)
        .arg("--out")
        .arg(out)
        .env("SOURCE_DATE_EPOCH", EXPORTED_AT)
        .output()?;
    assert!(result.status.success(), "catalog command: {result:?}");
    Ok(fs::read(out)?)
}

fn schema(options: &[&str]) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_str(&schema_output(options)?)?)
}

fn schema_output(options: &[&str]) -> Result<String, Box<dyn Error>> {
    let result = p4kx_testkit::p4kx()?
        .env("P4KX_ARCHIVE_KEY", "an invalid key")
        .args(["catalog", "schema"])
        .args(options)
        .output()?;
    assert!(
        result.status.success() && result.stderr.is_empty(),
        "catalog schema exit {}: {}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(String::from_utf8(result.stdout)?)
}
