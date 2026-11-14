// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use jsonschema::Validator;

use p4kx_extraction::{
    FieldOrigin, FieldOriginSeed, InputScope, OriginReaderLimits, SourceInput, SourceLocation,
    SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use super::TestResult;

#[derive(Clone, Copy)]
enum SchemaFlavor {
    Origin,
    SourcedValue,
}

impl SchemaFlavor {
    fn schema(self) -> Value {
        match self {
            Self::Origin => schemars::schema_for!(FieldOrigin).to_value(),
            Self::SourcedValue => schemars::schema_for!(SourcedValue<u32>).to_value(),
        }
    }

    fn wire(self, value: &SourcedValue<u32>) -> Result<Value, serde_json::Error> {
        match self {
            Self::Origin => serde_json::to_value(value.origin()),
            Self::SourcedValue => serde_json::to_value(value),
        }
    }
}

pub(super) fn registered_value_with_encryption(
    encrypted: bool,
) -> Result<SourcedValue<u32>, Box<dyn Error>> {
    let entry = TestEntry::stored("fixture.bin", b"data");
    let entry = if encrypted { entry.encrypted() } else { entry };
    let (_directory, archive) = super::super::open(&ArchiveBuilder::new().entry(entry))?;
    let entry = archive.entries().first().ok_or("fixture Entry")?;
    let origin = archive.entry_origin(entry).ok_or("actual Entry origin")?;
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(&origin), InputScope::Payload)?;
    let used = scope.at(
        SourceLocation::DataForge {
            record_index: 0,
            field: "value".into(),
        },
        SourceUseRole::Payload,
    )?;
    let value = sources.value(SourceValue::Present(0_u32), &[used], true);
    assert_eq!(
        value.origin().contains_encrypted_source(),
        Some(encrypted),
        "the control retains the actual Entry declaration"
    );
    assert_eq!(
        value.origin().source_uses_refs().len(),
        1,
        "the control has one actual registered contribution"
    );
    Ok(value)
}

#[test]
fn field_origin_schema_matches_the_owning_reader_contract() -> TestResult {
    check_schema(SchemaFlavor::Origin)
}

#[test]
fn sourced_value_schema_matches_the_owning_reader_contract() -> TestResult {
    check_schema(SchemaFlavor::SourcedValue)
}

fn check_schema(flavor: SchemaFlavor) -> TestResult {
    let validator = jsonschema::validator_for(&flavor.schema())?;
    let sources = SourceUses::new();
    let literal = sources.value(SourceValue::Present(0_u32), &[], true);
    let unknown = sources.value(SourceValue::Present(0_u32), &[], false);
    let plain = registered_value_with_encryption(false)?;
    let encrypted = registered_value_with_encryption(true)?;
    for (value, marker, complete, count) in [
        (&literal, json!(false), true, 0),
        (&unknown, Value::Null, false, 0),
        (&plain, json!(false), true, 1),
        (&encrypted, json!(true), true, 1),
    ] {
        let mut wire = flavor.wire(value)?;
        assert_eq!(
            wire.get("containsEncryptedSource")
                .ok_or("encryption marker")?,
            &marker,
            "the control carries the expected encryption marker"
        );
        assert_eq!(
            wire.get("sourceSetComplete").ok_or("source completeness")?,
            complete,
            "the control carries its actual contributor completeness"
        );
        assert_eq!(
            wire.get("sourceUsesRefs")
                .and_then(Value::as_array)
                .ok_or("references")?
                .len(),
            count,
            "the control retains all actual contribution references"
        );
        assert!(validator.is_valid(&wire), "actual producer control: {wire}");
        assert!(
            read_origin(&wire).is_ok(),
            "the owning reader accepts the control"
        );
        wire.as_object_mut()
            .ok_or("origin object")?
            .insert("futureMetadata".into(), json!({"preserved": true}));
        assert!(validator.is_valid(&wire), "additive metadata remains valid");
        assert!(
            read_origin(&wire).is_ok(),
            "the reader accepts additive metadata"
        );
    }
    check_rejections(&validator, flavor.wire(&plain)?)
}

fn check_rejections(validator: &Validator, mut wire: Value) -> TestResult {
    let reference = wire
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .and_then(|references| references.first())
        .ok_or("registered contribution reference")?
        .clone();
    assert!(
        reference.as_str().is_some_and(|value| !value.is_empty()),
        "the negative cases retain a real, nonempty registered reference"
    );
    for (marker, complete, references) in [
        (json!(true), true, json!([])),
        (json!(true), false, json!([])),
        (Value::Null, true, json!([])),
        (json!(false), false, json!([&reference])),
        (json!(false), true, json!([&reference, &reference])),
        (json!(false), true, json!([""])),
    ] {
        *wire
            .get_mut("containsEncryptedSource")
            .ok_or("encryption marker")? = marker;
        *wire
            .get_mut("sourceSetComplete")
            .ok_or("source completeness")? = json!(complete);
        *wire.get_mut("sourceUsesRefs").ok_or("references")? = references;
        assert!(
            !validator.is_valid(&wire),
            "schema rejects the invalid origin: {wire}"
        );
        assert!(
            read_origin(&wire).is_err(),
            "owning reader rejects the same origin: {wire}"
        );
    }
    Ok(())
}

fn read_origin(wire: &Value) -> Result<FieldOrigin, serde_json::Error> {
    use serde::de::DeserializeSeed as _;

    FieldOriginSeed::new(OriginReaderLimits {
        fields: 1,
        metadata_bytes: 16 << 10,
        path_bytes: 1024,
        field_uses: 8,
        reference_bytes: 1024,
    })
    .deserialize(wire)
}
