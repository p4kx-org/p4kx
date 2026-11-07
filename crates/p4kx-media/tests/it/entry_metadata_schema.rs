// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::EntryMetadataInfo;
use p4kx_media::{ExportOptions, Index, MediaKind, MediaRecord};
use serde_json::{Value, json};

use super::support::{self, TestResult};

fn exported_archive() -> Result<(Index, Vec<MediaRecord>), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    let options = ExportOptions {
        kinds: vec![MediaKind::Archive],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "archive.jsonl")?;
    Ok((index, rows))
}

#[test]
fn archive_provider_exports_directory_words_for_each_physical_entry() -> TestResult {
    let (index, rows) = exported_archive()?;
    let record_schema = p4kx_media::builtin_record_schema()?;
    let record_validator = jsonschema::validator_for(&record_schema)?;
    for row in &rows {
        record_validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    let metadata: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "archive-entry")
        .map(|row| {
            row.data
                .get("metadata")
                .ok_or("exported directory metadata")
        })
        .collect::<Result<_, _>>()?;
    assert_eq!(
        metadata.len(),
        3,
        "duplicate names retain separate metadata owners"
    );
    let schema = schemars::schema_for!(EntryMetadataInfo).to_value();
    let validator = jsonschema::validator_for(&schema)?;
    for (index, actual) in metadata.into_iter().enumerate() {
        assert_eq!(
            actual,
            &json!({
                "owner_entry_index": index,
                "dos_timestamp": {"raw_date": 0, "raw_time": 0, "calendar": {"state": "invalid"}},
                "record_format": "zip_central_directory",
                "original_directory_ordinal": index.to_string(),
                "raw_compression_method": if index == 1 { 100 } else { 0 },
                "zip_header": {"raw_version_made_by": 45, "raw_version_needed": 45,
                    "general_purpose_flags": 0, "disk_start": 65535,
                    "internal_attributes": 0, "external_attributes": 0},
                "raw_v2_encryption_word": null
            }),
            "raw zero timestamp words remain distinct from missing metadata and calendar defaults"
        );
        validator
            .validate(actual)
            .map_err(|error| error.to_string())?;
    }
    assert!(
        index.complete,
        "invalid calendar words do not create an integrity failure"
    );
    Ok(())
}

fn archive_record() -> Result<Value, Box<dyn std::error::Error>> {
    let (_, rows) = exported_archive()?;
    let row = rows
        .into_iter()
        .find(|row| row.subresource.namespace == "archive-entry")
        .ok_or("actual Archive Entry record")?;
    Ok(serde_json::to_value(row)?)
}

fn valid_metadata(second: u16) -> Value {
    json!({
        "owner_entry_index": 0,
        "record_format": "zip_central_directory", "original_directory_ordinal": "0",
        "raw_compression_method": 0,
        "zip_header": {"raw_version_made_by": 45, "raw_version_needed": 45,
            "general_purpose_flags": 0, "disk_start": 65535,
            "internal_attributes": 0, "external_attributes": 0},
        "raw_v2_encryption_word": null,
        "dos_timestamp": {
            "raw_date": 0x285d_u16,
            "raw_time": 0xbf60_u16 + second / 2,
            "calendar": {
                "state": "valid", "year": 2000, "month": 2, "day": 29,
                "hour": 23, "minute": 59, "second": second
            }
        }
    })
}

#[test]
fn timestamp_schema_enforces_even_seconds_and_complete_unsigned_source_words() -> TestResult {
    let schema = schemars::schema_for!(EntryMetadataInfo).to_value();
    let validator = jsonschema::validator_for(&schema)?;
    for second in [0, 58] {
        assert!(
            validator.is_valid(&valid_metadata(second)),
            "valid DOS clock boundary"
        );
    }
    for second in [1, 57, 60] {
        assert!(
            !validator.is_valid(&valid_metadata(second)),
            "unrepresentable DOS second"
        );
    }
    for value in [json!(-1), json!(65_536), json!("0"), Value::Null] {
        for pointer in ["/dos_timestamp/raw_date", "/dos_timestamp/raw_time"] {
            let mut mutation = valid_metadata(0);
            *mutation.pointer_mut(pointer).ok_or("raw word")? = value.clone();
            assert!(
                !validator.is_valid(&mutation),
                "raw source words have exact u16 bounds"
            );
        }
    }
    for word in [0_u16, u16::MAX] {
        let invalid_calendar = json!({
            "owner_entry_index": 0,
            "dos_timestamp": {"raw_date": word, "raw_time": word, "calendar": {"state": "invalid"}},
            "record_format": "zip_central_directory", "original_directory_ordinal": null,
            "raw_compression_method": 0,
            "zip_header": {"raw_version_made_by": 45, "raw_version_needed": 45,
                "general_purpose_flags": 0, "disk_start": 65535,
                "internal_attributes": 0, "external_attributes": 0},
            "raw_v2_encryption_word": null
        });
        assert!(
            validator.is_valid(&invalid_calendar),
            "invalid raw words remain recoverable"
        );
    }
    Ok(())
}

#[test]
fn builtin_archive_metadata_requires_a_typed_object_and_valid_source_widths() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    let mut record = archive_record()?;
    for invalid in [json!({}), json!({"metadata": null}), Value::Null] {
        *record.get_mut("data").ok_or("record data")? = invalid;
        assert!(!validator.is_valid(&record), "owned metadata is required");
    }
    *record.get_mut("data").ok_or("record data")? = json!({"metadata": valid_metadata(0)});
    for second in [0, 58] {
        *record.pointer_mut("/data/metadata").ok_or("metadata")? = valid_metadata(second);
        assert!(validator.is_valid(&record), "valid DOS clock boundary");
    }
    for second in [1, 57, 60] {
        *record.pointer_mut("/data/metadata").ok_or("metadata")? = valid_metadata(second);
        assert!(!validator.is_valid(&record), "unrepresentable DOS second");
    }
    for value in [json!(-1), json!(65_536), json!("0"), Value::Null] {
        for field in ["raw_date", "raw_time"] {
            *record.pointer_mut("/data/metadata").ok_or("metadata")? = valid_metadata(0);
            *record
                .pointer_mut(&format!("/data/metadata/dos_timestamp/{field}"))
                .ok_or("raw source word")? = value.clone();
            assert!(
                !validator.is_valid(&record),
                "exact unsigned16 source width"
            );
        }
    }
    Ok(())
}

#[test]
fn archive_binding_keeps_foreign_kinds_and_custom_namespaces_generic() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    let mut record = archive_record()?;
    *record.get_mut("data").ok_or("record data")? = json!({});
    assert!(!validator.is_valid(&record), "owned archive-entry metadata");
    for namespace in ["custom.archive", "archive-root"] {
        *record
            .pointer_mut("/subresource/namespace")
            .ok_or("namespace")? = json!(namespace);
        validator
            .validate(&record)
            .map_err(|error| error.to_string())?;
    }
    *record
        .pointer_mut("/subresource/namespace")
        .ok_or("namespace")? = json!("archive-entry");
    *record.get_mut("kind").ok_or("record kind")? = json!("shader");
    validator
        .validate(&record)
        .map_err(|error| error.to_string())?;
    Ok(())
}
