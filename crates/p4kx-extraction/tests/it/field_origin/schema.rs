// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    ArchiveByteRange, FieldOrigin, InputScope, SourceInput, SourceInventory, SourceUses,
    SourceValue, SourceValueState, SourcedValue,
};
use serde_json::{Value, json};

use super::TestResult;

#[test]
fn nonpresent_states_require_null_and_all_immediate_fields() -> TestResult {
    let schema = schemars::schema_for!(SourcedValue<u32>);
    let validator = jsonschema::validator_for(&serde_json::to_value(schema)?)?;
    for state in [
        SourceValue::Absent,
        SourceValue::Null,
        SourceValue::Unreadable,
        SourceValue::Unsupported,
        SourceValue::Invalid,
    ] {
        let value = SourceUses::new().value::<u32>(state, &[], true);
        let original = serde_json::to_value(value)?;
        assert!(
            validator.is_valid(&original),
            "the typed nonpresent carrier validates"
        );
        for wrong in [json!(1), json!(false), json!(""), json!([]), json!({})] {
            let mut changed = original.clone();
            changed["value"] = wrong;
            assert!(
                !validator.is_valid(&changed),
                "a nonpresent state cannot carry a value"
            );
        }
        for field in [
            "state",
            "value",
            "containsEncryptedSource",
            "sourceUsesRefs",
            "sourceSetComplete",
        ] {
            let mut changed = original.clone();
            changed
                .as_object_mut()
                .ok_or("carrier object")?
                .remove(field);
            assert!(
                !validator.is_valid(&changed),
                "required carrier field cannot disappear: {field}"
            );
        }
    }
    Ok(())
}

#[test]
fn present_values_follow_the_concrete_generic_schema_including_nullable_t() -> TestResult {
    let value = SourceUses::new().value(SourceValue::Present(0_u32), &[], true);
    let original = serde_json::to_value(value)?;
    let validator = jsonschema::validator_for(&serde_json::to_value(schemars::schema_for!(
        SourcedValue<u32>
    ))?)?;
    assert!(
        validator.is_valid(&original),
        "present zero remains a real value"
    );
    for wrong in [json!(null), json!(false), json!(""), json!([]), json!({})] {
        let mut changed = original.clone();
        changed["value"] = wrong;
        assert!(
            !validator.is_valid(&changed),
            "present u32 cannot accept another payload type"
        );
    }
    let nullable = SourceUses::new().value(SourceValue::Present(None::<u32>), &[], true);
    let nullable = serde_json::to_value(nullable)?;
    let validator = jsonschema::validator_for(&serde_json::to_value(schemars::schema_for!(
        SourcedValue<Option<u32>>
    ))?)?;
    assert!(
        validator.is_valid(&nullable),
        "present nullable T may itself serialize as null"
    );
    let mut wrong = nullable;
    wrong["value"] = json!("not a number");
    assert!(
        !validator.is_valid(&wrong),
        "nullable T does not erase its non-null type"
    );
    Ok(())
}

#[test]
fn present_false_empty_text_and_empty_lists_keep_their_concrete_types() -> TestResult {
    fn valid<T: schemars::JsonSchema + serde::Serialize>(value: T) -> TestResult {
        let value = SourceUses::new().value(SourceValue::Present(value), &[], true);
        let original = serde_json::to_value(value)?;
        let validator = jsonschema::validator_for(&serde_json::to_value(schemars::schema_for!(
            SourcedValue<T>
        ))?)?;
        assert!(
            validator.is_valid(&original),
            "a typed present empty value validates"
        );
        let mut wrong = original;
        wrong["value"] = json!({});
        assert!(
            !validator.is_valid(&wrong),
            "the concrete generic type stays enforced"
        );
        Ok(())
    }
    valid(false)?;
    valid(String::new())?;
    valid(Vec::<u32>::new())
}

#[test]
fn fallible_mapping_propagates_errors_and_skips_nonpresent_values() -> TestResult {
    let source = SourceUses::new();
    let present = source.value(SourceValue::Present(1_u32), &[], true);
    let result = present.try_map(|_| Err::<u32, _>("projection failed"));
    assert!(
        matches!(result, Err("projection failed")),
        "the projection error is not replaced"
    );
    let absent = source.value(SourceValue::<u32>::Absent, &[], true);
    let result = absent.try_map(|_| Err::<u32, _>("must not run"))?;
    assert_eq!(
        result.state(),
        SourceValueState::Absent,
        "absent mapping retains its state"
    );
    assert!(
        result.value().is_none(),
        "absent mapping does not invent a payload"
    );
    Ok(())
}

#[test]
fn origin_schema_rejects_unencrypted_claims_for_incomplete_sources() -> TestResult {
    let value = SourceUses::new().value(SourceValue::Present(0_u32), &[], true);
    check_filter_index(
        &serde_json::to_value(schemars::schema_for!(FieldOrigin))?,
        &serde_json::to_value(value.origin())?,
    )
}

#[test]
fn sourced_value_schema_retains_the_origin_filter_constraint_when_flattened() -> TestResult {
    let value = SourceUses::new().value(SourceValue::Present(0_u32), &[], true);
    check_filter_index(
        &serde_json::to_value(schemars::schema_for!(SourcedValue<u32>))?,
        &serde_json::to_value(value)?,
    )
}

fn check_filter_index(schema: &Value, original: &Value) -> TestResult {
    let validator = jsonschema::validator_for(schema)?;
    assert!(
        validator.is_valid(original),
        "the actual typed origin fixture validates"
    );
    for (encrypted, complete, allowed) in [
        (json!(false), false, false),
        (json!(false), true, true),
        (json!(true), false, true),
        (json!(true), true, true),
        (Value::Null, false, true),
        (Value::Null, true, true),
    ] {
        let mut changed = original.clone();
        *changed
            .get_mut("containsEncryptedSource")
            .ok_or("encryption marker missing")? = encrypted;
        *changed
            .get_mut("sourceSetComplete")
            .ok_or("source completeness missing")? = json!(complete);
        assert_eq!(
            validator.is_valid(&changed),
            allowed,
            "only a false encryption claim requires a complete source set: {changed}"
        );
    }
    Ok(())
}

#[test]
fn shared_archive_ranges_stay_closed_while_inventory_metadata_stays_extensible() -> TestResult {
    let (_directory, archive) = super::open(
        &p4kx_testkit::ArchiveBuilder::new()
            .entry(p4kx_testkit::TestEntry::stored("fixture.bin", b"data")),
    )?;
    let entry = archive.entries().first().ok_or("fixture Entry")?;
    let origin = archive.entry_origin(entry).ok_or("actual Entry origin")?;
    let sources = SourceUses::new();
    let _scope = sources.observe(SourceInput::Archive(&origin), InputScope::Payload)?;
    let original = serde_json::to_value(sources.freeze()?)?;
    let range = original
        .pointer("/sources/0/recordRange")
        .ok_or("actual source range")?;
    let range_schema = serde_json::to_value(schemars::schema_for!(ArchiveByteRange))?;
    let range_validator = jsonschema::validator_for(&range_schema)?;
    let inventory_schema = serde_json::to_value(schemars::schema_for!(SourceInventory))?;
    let inventory_validator = jsonschema::validator_for(&inventory_schema)?;
    assert!(
        range_validator.is_valid(range),
        "the actual shared range validates"
    );
    assert!(
        inventory_validator.is_valid(&original),
        "the actual inventory validates"
    );
    let mut extended_range = range.clone();
    extended_range
        .as_object_mut()
        .ok_or("range object")?
        .insert("futureMetadata".into(), json!(true));
    assert!(
        !range_validator.is_valid(&extended_range),
        "the shared range contract still rejects unknown fields for existing consumers"
    );
    let mut extended_inventory = original;
    *extended_inventory
        .pointer_mut("/sources/0/recordRange")
        .ok_or("source range")? = extended_range;
    assert!(
        inventory_validator.is_valid(&extended_inventory),
        "only the new inventory range schema admits future metadata"
    );
    Ok(())
}
