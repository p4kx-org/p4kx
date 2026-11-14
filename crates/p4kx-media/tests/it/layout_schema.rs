// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_media::{ExportOptions, MediaRecord};
use serde_json::json;

use super::layout_fixture;
use super::support::{self, TestResult};

#[test]
fn builtin_schema_requires_exact_version_presence_and_lossless_source_offsets() -> TestResult {
    let folder = tempfile::tempdir()?;
    let fixture = layout_fixture::zip(&[])?;
    let archive = layout_fixture::open(folder.path(), &fixture.bytes, false)?;
    let output = folder.path().join("media");
    let index = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    let rows = support::records(&output, &index, "archive.jsonl")?;
    let schema = p4kx_media::builtin_record_schema()?;
    let validator = jsonschema::validator_for(&schema)?;
    let record = super::layout_support::row(&rows, "archive-directory-record")?;
    let original = serde_json::to_value(record)?;
    assert!(validator.is_valid(&original));
    assert_required_fields(&validator, &original)?;
    for value in [json!("9007199254740993"), json!("18446744073709551615")] {
        let mut good = original.clone();
        *good
            .pointer_mut("/data/raw/range/offset")
            .ok_or("source offset")? = value;
        assert!(
            validator.is_valid(&good),
            "source width is not a JSON floating-point bound"
        );
    }
    for value in [
        json!("18446744073709551616"),
        json!("01"),
        json!("1\n"),
        json!(1),
        json!(null),
    ] {
        let mut bad = original.clone();
        *bad.pointer_mut("/data/raw/range/offset")
            .ok_or("source offset")? = value;
        assert!(
            !validator.is_valid(&bad),
            "invalid exact unsigned source word"
        );
    }
    for (path, value) in [
        ("/data/version", json!("2.0")),
        ("/data/name", json!(7)),
        ("/data/unknown", json!(false)),
    ] {
        let mut bad = original.clone();
        if path == "/data/unknown" {
            bad.get_mut("data")
                .and_then(serde_json::Value::as_object_mut)
                .ok_or("record data")?
                .insert("unknown".into(), value);
        } else {
            *bad.pointer_mut(path).ok_or("mutated field")? = value;
        }
        assert!(!validator.is_valid(&bad), "payload mutation {path}");
    }
    Ok(())
}

#[test]
fn namespace_bindings_preserve_custom_providers_and_other_media_kinds() -> TestResult {
    let schema = p4kx_media::builtin_record_schema()?;
    let validator = jsonschema::validator_for(&schema)?;
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    let index = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    let rows = support::records(&output, &index, "archive.jsonl")?;
    let row = super::layout_support::row(&rows, "archive-layout")?;
    for (kind, namespace) in [("archive", "custom-layout"), ("shader", "archive-layout")] {
        let mut custom = serde_json::to_value(row)?;
        *custom.get_mut("kind").ok_or("kind")? = json!(kind);
        *custom
            .pointer_mut("/subresource/namespace")
            .ok_or("namespace")? = json!(namespace);
        *custom.get_mut("data").ok_or("data")? = json!({"custom": true});
        assert!(
            validator.is_valid(&custom),
            "unowned payload namespace remains generic"
        );
    }
    Ok(())
}

#[test]
fn archive_payload_fields_are_required_while_explicit_nulls_remain_valid() -> TestResult {
    let schema = p4kx_media::builtin_record_schema()?;
    let validator = jsonschema::validator_for(&schema)?;
    let namespaces = [
        "archive-layout",
        "archive-section",
        "archive-raw-chunk",
        "archive-directory-record",
        "archive-extra",
        "archive-installation",
        "archive-free-range",
    ];
    let mut seen = BTreeSet::new();
    let mut nullable = false;
    for fixture in [layout_fixture::zip(&[1, 1, 4, 0])?, layout_fixture::v2()?] {
        let folder = tempfile::tempdir()?;
        let archive = layout_fixture::open(folder.path(), &fixture.bytes, false)?;
        let output = folder.path().join("media");
        let index = support::published(p4kx_media::export(
            &archive,
            &output,
            &ExportOptions::default(),
        )?)?;
        for record in support::records(&output, &index, "archive.jsonl")? {
            if namespaces.contains(&record.subresource.namespace.as_str()) {
                nullable |= assert_payload_fields(&validator, &record)?;
                seen.insert(record.subresource.namespace);
            }
        }
    }
    assert_eq!(
        seen.len(),
        namespaces.len(),
        "every owned payload was exercised"
    );
    assert!(nullable, "explicit null source facts remain valid");
    let required = schema
        .pointer("/$defs/ArchiveMetadataDiagnostic/required")
        .and_then(serde_json::Value::as_array)
        .ok_or("diagnostic presence contract")?;
    for field in ["version", "operation", "failure", "offset", "length"] {
        assert!(
            required.contains(&json!(field)),
            "required diagnostic field {field}"
        );
    }
    Ok(())
}

fn assert_payload_fields(
    validator: &jsonschema::Validator,
    record: &MediaRecord,
) -> Result<bool, Box<dyn std::error::Error>> {
    let original = serde_json::to_value(record)?;
    assert!(
        validator.is_valid(&original),
        "actual owned payload remains valid"
    );
    let data = original
        .get("data")
        .and_then(serde_json::Value::as_object)
        .ok_or("structured archive payload")?;
    for field in data.keys() {
        let mut missing = original.clone();
        missing
            .get_mut("data")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("structured archive payload")?
            .remove(field);
        assert!(
            !validator.is_valid(&missing),
            "{} requires its serialized {field} field",
            record.subresource.namespace
        );
    }
    Ok(data.values().any(serde_json::Value::is_null))
}

fn assert_required_fields(
    validator: &jsonschema::Validator,
    original: &serde_json::Value,
) -> TestResult {
    for field in [
        "version",
        "signature",
        "storedDigest",
        "metadata",
        "originalDirectoryOrdinal",
    ] {
        let mut bad = original.clone();
        bad.get_mut("data")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("record data")?
            .remove(field);
        assert!(!validator.is_valid(&bad), "required source field {field}");
    }
    Ok(())
}
