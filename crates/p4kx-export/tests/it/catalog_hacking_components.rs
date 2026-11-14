// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{
    Archive, DataForge, DataType, Field, RecordTree, StructValue, Value as Raw, ValueError,
};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::catalog_tags_fixture as tags;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_hacking_empty_target.rs"]
mod empty_target;
#[path = "catalog_hacking_components_fixture.rs"]
mod fixture;
use fixture::{CHARGES, Case, MODIFIERS};

const ROWS: &str = "/configurations/hackingChips/rows";

#[test]
fn hacking_components_keep_zero_charges_and_ordered_original_modifier_values() -> TestResult {
    let source = fixture::database(Case::Valid);
    let source_ordinals = with_tree(&source, original_modifiers)?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&source, origin)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        let [charges, modifiers, empty] = proof::rows(&catalog, ROWS)? else {
            return Err("one chip charge and two removable modifier declarations".into());
        };
        assert_eq!(
            proof::at(&catalog, "/configurations/hackingChips/availability")?,
            "available",
            "real chip source families are declared"
        );
        let charge = configuration(charges, 1, CHARGES, "hackingChip", encrypted)?;
        proof::present(charge, "maxCharges", &json!(0), Some(encrypted))?;
        check_item_without_specialist_stats(&catalog)?;
        let values = configuration(modifiers, 2, MODIFIERS, "removableChip", encrypted)?;
        let values = proof::rows(values, "/values/value")?;
        assert_eq!(
            values.len(),
            fixture::NAMES.len(),
            "duplicates and unknown names survive"
        );
        for ((entry, (name, number)), ordinal) in values
            .iter()
            .zip(fixture::NAMES.into_iter().zip(fixture::SINGLES))
            .zip(source_ordinals.iter())
        {
            check_modifier(&catalog, entry, name, number, *ordinal, encrypted)?;
        }
        let empty = configuration(empty, 3, MODIFIERS, "removableChip", encrypted)?;
        proof::present(empty, "values", &json!([]), Some(encrypted))?;
        check_entity_tag(&catalog, tags::ROOT, "present", encrypted)?;
    }
    Ok(())
}

#[test]
fn hacking_components_reject_non_class_modifier_arrays_at_the_original_declaration() -> TestResult {
    let source = fixture::database(Case::WrongStorage);
    with_tree(&source, |tree| {
        let modifiers = component(tree, 2)?;
        let field = modifiers
            .fields()
            .iter()
            .find(|field| field.name() == "values")
            .ok_or("original values declaration")?;
        assert_eq!(
            field.storage().data_type(),
            DataType::StrongPointer,
            "actually wrong storage"
        );
        assert!(
            matches!(field.value(), Raw::Array(values) if values.len() == 3),
            "wrong storage still decodes to a plausible array"
        );
        Ok(())
    })?;
    let (_folder, archive) = common::open(&source, DcbOrigin::Encrypted)?;
    let catalog = diagnostic(&archive)?;
    let row = proof::rows(&catalog, ROWS)?
        .get(1)
        .ok_or("original removable chip")?;
    let config = configuration(row, 2, MODIFIERS, "removableChip", true)?;
    let values = proof::cell(config, "values", "invalid")?;
    assert!(
        proof::uses(&catalog, values)?.iter().any(|entry| entry
            .pointer("/location/field")
            .and_then(Value::as_str)
            == Some("$dataType")
            && entry.get("role").and_then(Value::as_str) == Some("selector")),
        "the original non-Class storage is the actual rejection reason"
    );
    Ok(())
}

#[test]
fn hacking_components_preserve_later_modifiers_and_separate_dangling_entity_tags() -> TestResult {
    let source = fixture::database(Case::Unreadable);
    with_tree(&source, |tree| {
        let values = component(tree, 2)?
            .field("values")
            .ok_or("original values")?;
        let Raw::Array(values) = values else {
            return Err("actual array".into());
        };
        let first = structure(values.first().ok_or("original first modifier")?)?;
        assert!(
            matches!(first.field("name"), Some(Raw::Error(ValueError::InvalidString { offset, .. }))
            if *offset == fixture::BAD_OFFSET),
            "the first original name is unreadable"
        );
        let Some(Raw::Array(tags)) = tree.field("tags") else {
            return Err("original tag slots".into());
        };
        assert!(
            matches!(tags.first(), Some(Raw::Error(ValueError::DanglingReference { guid }))
            if guid.to_string() == common::guid(tags::MISSING)),
            "the original access tag is dangling"
        );
        Ok(())
    })?;
    let (_folder, archive) = common::open(&source, DcbOrigin::Encrypted)?;
    let catalog = diagnostic(&archive)?;
    let row = proof::rows(&catalog, ROWS)?
        .get(1)
        .ok_or("original removable chip")?;
    let config = configuration(row, 2, MODIFIERS, "removableChip", true)?;
    let [first, second, third, fourth] = proof::rows(config, "/values/value")? else {
        return Err("one unreadable name leaves all four original modifier slots".into());
    };
    proof::cell(proof::at(first, "/value")?, "name", "unreadable")?;
    for row in [second, third, fourth] {
        proof::cell(proof::at(row, "/value")?, "name", "present")?;
    }
    check_entity_tag(&catalog, tags::MISSING, "dangling", true)
}

fn original_modifiers(tree: &RecordTree<'_>) -> TestResult<Vec<u64>> {
    let charges = component(tree, 1)?;
    assert!(
        matches!(charges.field("maxCharges"), Some(Raw::Int32(0))),
        "the source stores zero Int32"
    );
    assert!(
        charges.field("errorChance").is_none(),
        "this original charge type declares no error chance"
    );
    let modifiers = component(tree, 2)?;
    let field = modifiers
        .fields()
        .iter()
        .find(|field| field.name() == "values")
        .ok_or("original values declaration")?;
    assert_eq!(
        field.storage().data_type(),
        DataType::Class,
        "real inline array declaration"
    );
    let Raw::Array(values) = field.value() else {
        return Err("actual modifier list".into());
    };
    assert_eq!(
        values.len(),
        fixture::NAMES.len(),
        "exact original slot count"
    );
    values
        .iter()
        .zip(fixture::NAMES.into_iter().zip(fixture::SINGLES))
        .map(|(original, (name, number))| {
            let original = structure(original)?;
            assert!(
                matches!(original.field("name"), Some(Raw::String(actual)) if *actual == name),
                "actual original modifier name"
            );
            let value = original.field("value").ok_or("actual modifier value")?;
            assert!(
                matches!(value, Raw::Single(actual) if actual.to_bits() == number.to_bits()),
                "actual original Single bits"
            );
            source_ordinal(tree, value)
        })
        .collect()
}

fn check_modifier(
    catalog: &Value,
    entry: &Value,
    name: &str,
    number: f32,
    ordinal: u64,
    encrypted: bool,
) -> TestResult {
    let fields = proof::at(entry, "/value")?;
    proof::present(
        fields,
        "name",
        &json!({"type": "gameText", "sourceText": name}),
        Some(encrypted),
    )?;
    let value = proof::present(
        fields,
        "value",
        &json!({"rawBits": format!("{:08x}", number.to_bits()), "finite": number}),
        Some(encrypted),
    )?;
    let expected = json!(["0", ordinal.to_string()]);
    assert!(
        proof::uses(catalog, value)?.iter().any(|entry| entry
            .pointer("/location/namespace")
            .and_then(Value::as_str)
            == Some("dataforgeValue")
            && entry.pointer("/location/field").and_then(Value::as_str) == Some("$value")
            && entry.pointer("/location/indices") == Some(&expected)
            && entry.get("role").and_then(Value::as_str) == Some("payload")),
        "each equal-name modifier keeps its independently enumerated payload occurrence"
    );
    Ok(())
}

fn check_item_without_specialist_stats(catalog: &Value) -> TestResult {
    let owner = common::guid(fixture::OWNER);
    let item = proof::rows(catalog, "/items")?
        .iter()
        .find(|row| row.get("guid").and_then(Value::as_str) == Some(owner.as_str()))
        .ok_or("item owning the original chip")?;
    assert!(
        proof::at(item, "/stats")?.is_null(),
        "the independent chip requires no specialist stats"
    );
    Ok(())
}

fn check_entity_tag(catalog: &Value, target: u128, status: &str, encrypted: bool) -> TestResult {
    let owner = common::guid(fixture::OWNER);
    let row = proof::rows(catalog, "/declarations/entityTags/rows")?
        .iter()
        .find(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(owner.as_str()))
        .ok_or("physical chip Entity tags")?;
    proof::present(row, "recordIndex", &json!(0), Some(encrypted))?;
    let [tag] = proof::rows(row, "/tags/value")? else {
        return Err("one authored tag".into());
    };
    let original = proof::at(tag, "/value")?;
    proof::present(
        original,
        "guid",
        &json!(common::guid(target)),
        Some(encrypted),
    )?;
    assert_eq!(
        proof::at(original, "/status")?,
        status,
        "root tag Reference status stays separate"
    );
    Ok(())
}

fn configuration<'row>(
    row: &'row Value,
    slot: u32,
    source: &str,
    kind: &str,
    encrypted: bool,
) -> TestResult<&'row Value> {
    proof::present(
        row,
        "ownerGuid",
        &json!(common::guid(fixture::OWNER)),
        Some(encrypted),
    )?;
    proof::present(row, "ownerRecordIndex", &json!(0), Some(encrypted))?;
    proof::present(row, "componentOrdinal", &json!(slot), Some(encrypted))?;
    proof::present(row, "sourceType", &json!(source), Some(encrypted))?;
    let config = proof::cell(row, "configuration", "present")?;
    assert_eq!(
        proof::at(config, "/value/type")?,
        kind,
        "the original concrete chip type"
    );
    proof::at(config, "/value/value")
}

fn diagnostic(archive: &Archive) -> TestResult<Value> {
    assert!(
        matches!(
            build_catalog(archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "normal publication refuses incomplete source"
    );
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        build_catalog(archive, &options)?.json(),
    )?)
}

fn with_tree<T>(
    source: &DataForgeBuilder,
    check: impl FnOnce(&RecordTree<'_>) -> TestResult<T>,
) -> TestResult<T> {
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(0).ok_or("physical chip owner")?.tree()?;
    check(&tree)
}

fn component<'read, 'source>(
    tree: &'read RecordTree<'source>,
    slot: usize,
) -> TestResult<&'read StructValue<'source>> {
    let Some(Raw::Array(components)) = tree.field("Components") else {
        return Err("original Components".into());
    };
    structure(components.get(slot).ok_or("original component slot")?)
}

fn structure<'read, 'source>(raw: &'read Raw<'source>) -> TestResult<&'read StructValue<'source>> {
    let Raw::Struct(value) = raw else {
        return Err("original struct".into());
    };
    Ok(value)
}

fn source_ordinal<'source>(tree: &RecordTree<'source>, target: &Raw<'source>) -> TestResult<u64> {
    let mut pending: Vec<_> = tree.fields().iter().rev().map(Field::value).collect();
    let mut ordinal = 0_u64;
    while let Some(value) = pending.pop() {
        if std::ptr::eq(value, target) {
            return Ok(ordinal);
        }
        ordinal = ordinal.checked_add(1).ok_or("source ordinal overflow")?;
        match value {
            Raw::Struct(owner) => pending.extend(owner.fields().iter().rev().map(Field::value)),
            Raw::Array(values) => pending.extend(values.iter().rev()),
            _ => {}
        }
    }
    Err("actual modifier has no original occurrence".into())
}
