// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogRecordSelection, SourceArenaOutput, SourceNode, SourcePayload, SourceValueRef,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::sdk_full_proof as full_proof;

use super::{MODIFIER, MODIFIER_PRIORITY, database_with_modifiers};

const PRIORITIES: [u32; 2] = [17, 3];

#[test]
fn populated_tagged_modifiers_preserve_order_types_and_source_values() -> TestResult {
    let (_folder, archive) =
        common::open(&database_with_modifiers(&PRIORITIES), DcbOrigin::Encrypted)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, options.with_full_provenance())?.json())?;
    let expected = proof::rows(&catalog, "/definitions/mineableEntities/rows")?
        .iter()
        .find(|row| row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(0))
        .ok_or("the original tagged mineable in the Catalog")?;
    full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_mineable_entities(
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("physical selection must return the single tagged owner".into());
            };
            let inventory = json!({"provenance": snapshot.source_inventory()});
            assert_eq!(
                proof::with_origins(serde_json::to_value(row)?, &inventory)?,
                proof::with_origins(expected.clone(), &catalog)?,
                "both projections retain the populated array and its original contributors"
            );
            let [component] = row.radar.as_slice() else {
                return Err("the tagged owner must have one radar component".into());
            };
            let properties = component
                .properties
                .value()
                .ok_or("original radar properties")?;
            verify_tagged_arena(&properties.authored_fields, &inventory)
        },
    )?
}

fn verify_tagged_arena(arena: &SourceArenaOutput<'_>, inventory: &Value) -> TestResult {
    let root = arena
        .roots()
        .iter()
        .find(|field| field.name().value().copied() == Some("taggedEmissionModifierParams"))
        .ok_or("original tagged modifier declaration")?;
    let array = arena_node(arena, root.value())?.value();
    let Some(SourcePayload::Array { elements }) = array.value() else {
        return Err("the authored tagged modifier declaration is an array".into());
    };
    assert_eq!(
        elements.len(),
        PRIORITIES.len(),
        "the authored array keeps both original class elements"
    );
    for (reference, expected) in elements.iter().zip(PRIORITIES) {
        verify_tagged_element(arena, inventory, *reference, expected)?;
    }
    Ok(())
}

fn verify_tagged_element(
    arena: &SourceArenaOutput<'_>,
    inventory: &Value,
    reference: SourceValueRef,
    expected: u32,
) -> TestResult {
    let Some(SourcePayload::Struct {
        source_type,
        fields,
    }) = arena_node(arena, reference)?.value().value()
    else {
        return Err("a populated modifier entry must remain a typed structure".into());
    };
    assert_eq!(
        source_type.value().copied(),
        Some(MODIFIER),
        "the modifier retains its concrete class-array element type"
    );
    let [priority] = fields.as_slice() else {
        return Err("the synthetic modifier has one original field".into());
    };
    assert_eq!(
        priority.name().value().copied(),
        Some(MODIFIER_PRIORITY),
        "the modifier's original field is preserved"
    );
    let scalar = arena_node(arena, priority.value())?.value();
    let Some(SourcePayload::Number { value }) = scalar.value() else {
        return Err("the modifier's priority must remain a number".into());
    };
    assert_eq!(
        serde_json::to_value(value)?,
        json!({"type": "uint32", "value": expected}),
        "the value retains its kind and the original position in the array"
    );
    let carrier = serde_json::to_value(scalar)?;
    assert_eq!(
        carrier
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "the individual authored modifier value is sourced from the encrypted database"
    );
    let uses = proof::uses(inventory, &carrier)?;
    assert!(
        uses.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some("payload")
                && used.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
                && used.pointer("/location/indices/0").and_then(Value::as_str) == Some("0")
                && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
        }),
        "the modifier value retains the physical payload in the original owner"
    );
    Ok(())
}

fn arena_node<'a, 'source>(
    arena: &'a SourceArenaOutput<'source>,
    reference: SourceValueRef,
) -> TestResult<&'a SourceNode<'source>> {
    arena
        .values()
        .get(usize::try_from(reference.value_index())?)
        .ok_or_else(|| "closed owner-local authored value reference".into())
}
