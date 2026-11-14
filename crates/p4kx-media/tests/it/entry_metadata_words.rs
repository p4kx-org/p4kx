// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, EntryMetadataInfo};
use p4kx_media::ShaderWord;
use schemars::{JsonSchema, SchemaGenerator};
use serde_json::{Value, json};

use super::support::TestResult;

#[test]
fn decimal_u64_schema_preserves_the_shader_word_contract_and_full_source_width() -> TestResult {
    let mut generator = SchemaGenerator::default();
    assert_eq!(
        ShaderWord::json_schema(&mut generator),
        DecimalU64::json_schema(&mut generator),
        "both source-word types share the same exact schema"
    );
    let schema = schemars::schema_for!(DecimalU64).to_value();
    let validator = jsonschema::validator_for(&schema)?;
    for word in [
        "0",
        "9007199254740993",
        "18446744073709551614",
        "18446744073709551615",
    ] {
        assert!(
            validator.is_valid(&json!(word)),
            "valid unsigned source word {word}"
        );
    }
    for word in [
        json!("18446744073709551616"),
        json!("99999999999999999999"),
        json!("-1"),
        json!("01"),
        json!("+1"),
        json!("1\n"),
        json!("1.0"),
        json!(u64::MAX),
        Value::Null,
    ] {
        assert!(!validator.is_valid(&word), "invalid source word {word}");
    }
    Ok(())
}

fn zip_metadata() -> Value {
    json!({
        "owner_entry_index": 0,
        "dos_timestamp": {"raw_date": 0, "raw_time": 0, "calendar": {"state": "invalid"}},
        "record_format": "zip_central_directory", "original_directory_ordinal": "18446744073709551615",
        "raw_compression_method": 65535,
        "zip_header": {"raw_version_made_by": 65535, "raw_version_needed": 65535,
            "general_purpose_flags": 65535, "disk_start": 65535,
            "internal_attributes": 65535, "external_attributes": 4_294_967_295_u32},
        "raw_v2_encryption_word": null
    })
}

#[test]
fn metadata_schema_requires_explicit_absence_and_matching_format_fields() -> TestResult {
    let schema = schemars::schema_for!(EntryMetadataInfo).to_value();
    let validator = jsonschema::validator_for(&schema)?;
    assert!(
        validator.is_valid(&zip_metadata()),
        "all raw ZIP bits are allowed"
    );
    for name in [
        "original_directory_ordinal",
        "zip_header",
        "raw_v2_encryption_word",
    ] {
        let mut missing = zip_metadata();
        missing
            .as_object_mut()
            .ok_or("metadata object")?
            .remove(name);
        assert!(
            !validator.is_valid(&missing),
            "absence must be explicit: {name}"
        );
    }
    let mut v2 = zip_metadata();
    v2["record_format"] = json!("p4k_v2");
    assert!(!validator.is_valid(&v2), "ZIP fields cannot impersonate v2");
    v2["zip_header"] = Value::Null;
    assert!(
        !validator.is_valid(&v2),
        "v2 encryption zero is present, not absent"
    );
    v2["raw_v2_encryption_word"] = json!(0);
    assert!(validator.is_valid(&v2), "explicit v2 zero");
    v2["raw_v2_encryption_word"] = json!(65535);
    assert!(validator.is_valid(&v2), "unknown v2 bits are source facts");
    v2["original_directory_ordinal"] = Value::Null;
    assert!(
        validator.is_valid(&v2),
        "unavailable source ordinal is explicit"
    );
    Ok(())
}

#[test]
fn metadata_schema_rejects_narrowed_or_numeric_source_word_representations() -> TestResult {
    let schema = schemars::schema_for!(EntryMetadataInfo).to_value();
    let validator = jsonschema::validator_for(&schema)?;
    for pointer in [
        "/raw_compression_method",
        "/zip_header/raw_version_made_by",
        "/zip_header/raw_version_needed",
        "/zip_header/general_purpose_flags",
        "/zip_header/disk_start",
        "/zip_header/internal_attributes",
    ] {
        for invalid in [json!(-1), json!(65536), json!("0"), Value::Null] {
            let mut metadata = zip_metadata();
            *metadata.pointer_mut(pointer).ok_or("raw word")? = invalid;
            assert!(
                !validator.is_valid(&metadata),
                "unsigned u16 source word: {pointer}"
            );
        }
    }
    for invalid in [json!(-1), json!(4_294_967_296_u64), json!("0")] {
        let mut metadata = zip_metadata();
        metadata["zip_header"]["external_attributes"] = invalid;
        assert!(
            !validator.is_valid(&metadata),
            "unsigned u32 external attributes"
        );
    }
    for invalid in [json!(u64::MAX), json!("18446744073709551616"), json!("01")] {
        let mut metadata = zip_metadata();
        metadata["original_directory_ordinal"] = invalid;
        assert!(!validator.is_valid(&metadata), "exact decimal u64 ordinal");
    }
    Ok(())
}
