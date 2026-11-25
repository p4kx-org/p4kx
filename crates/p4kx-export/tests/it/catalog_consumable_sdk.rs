// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::equipment::component_definitions::ConsumableEffectParameters::{
    AddBuffEffect, Authored, Gas, Health, ModifyActorStatus, Resource,
};
use p4kx_export::catalog_queries::equipment::component_definitions::{
    ConsumableEffect, ConsumableSubtype, ConsumableSubtypeSnapshot, ConsumableTypeSnapshot,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DefinitionStatus, ResourceFloat32,
};
use p4kx_extraction::{Language, SourceInventory, SourceValueState, SourcedValue};
use serde_json::Value;

use super::{SUBTYPES, TYPES, fixture};
use crate::canonical_header::wire::{Wire, visit_leaf};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::sdk_full_proof as full_proof;

#[test]
fn selected_consumable_definitions_keep_typed_slots_and_text_domains() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Encrypted)?;
    let catalog = build_catalog(&archive, common::options().with_full_provenance())?;
    let expected: Value = serde_json::from_slice(catalog.json())?;
    let session = full_proof::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    parsed.with_consumable_types(CatalogRecordSelection::RecordIndex(3), None, |rows| {
        type_definition(&rows, &expected)
    })??;
    parsed.with_consumable_subtypes(CatalogRecordSelection::RecordIndex(0), None, |rows| {
        subtype_definition(&rows, &expected)
    })??;
    parsed.with_consumable_subtypes(
        CatalogRecordSelection::RecordIndex(1),
        None,
        |rows| -> TestResult {
            let [row] = rows.rows() else {
                return Err("the second physical subtype".into());
            };
            assert_eq!(row.tint_override().state(), SourceValueState::Null);
            visit_leaf(row.tint_override(), rows.source_inventory())
        },
    )??;
    Ok(())
}

fn type_definition(snapshot: &ConsumableTypeSnapshot<'_>, expected: &Value) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one physical type definition".into());
    };
    let [original] = proof::rows(expected, TYPES)? else {
        return Err("one canonical type definition".into());
    };
    let wire = Wire::new(original, snapshot.source_inventory());
    wire.raw(row)?;
    assert_eq!(
        snapshot.requested_languages(),
        [],
        "type definitions need no locales"
    );
    let slots = row.subtypes().value().ok_or("authored subtype slots")?;
    let [first, second] = slots.as_slice() else {
        return Err("both original subtype positions".into());
    };
    for (slot, target) in [(first, 1), (second, 0)] {
        let linked = slot.value().ok_or("original subtype Reference")?;
        assert!(
            matches!(linked.status(), DefinitionStatus::Present),
            "the fixture target resolves"
        );
        assert_eq!(
            linked.target_record_index().value(),
            Some(&target),
            "the stored link order is physical"
        );
        assert_eq!(
            slot.origin().contains_encrypted_source(),
            Some(true),
            "the Reference slot has direct origin"
        );
        visit_leaf(slot, snapshot.source_inventory())?;
    }
    Ok(())
}

fn subtype_definition(snapshot: &ConsumableSubtypeSnapshot<'_>, expected: &Value) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one physical subtype definition".into());
    };
    let original = proof::rows(expected, SUBTYPES)?
        .first()
        .ok_or("canonical subtype")?;
    let wire = Wire::new(original, snapshot.source_inventory());
    wire.raw(row)?;
    assert_eq!(
        snapshot.available_languages(),
        [Language::English],
        "only the requested locale is read"
    );
    let name = row
        .consumable_name()
        .value()
        .ok_or("public-name reference")?;
    let name = snapshot.public_name(name).ok_or("public-name definition")?;
    assert_eq!(
        name.key().value().copied(),
        Some("@resource_name"),
        "the short name retains its key"
    );
    assert_tint(row)?;
    let slots = row.effects().value().ok_or("authored effect slots")?;
    let health = assert_effects(slots, snapshot.source_inventory())?;
    let reference = health
        .effect_description()
        .value()
        .ok_or("protected-text reference")?;
    let text = snapshot
        .game_text(reference)
        .ok_or("protected text definition")?;
    assert_eq!(
        text.key().value().copied(),
        Some("@own_description"),
        "the protected text retains its key"
    );
    let translation = text
        .translations()
        .value()
        .and_then(|items| items.get("en"))
        .ok_or("only the selected English translation")?;
    assert_eq!(
        translation.source_text.value().map(AsRef::as_ref),
        Some("Own description"),
        "the selected source text is available"
    );
    visit_leaf(&translation.source_text, snapshot.source_inventory())
}

fn assert_tint(row: &ConsumableSubtype<'_>) -> TestResult {
    let tint = row.tint_override().value().ok_or("a typed tint owner")?;
    assert_eq!(
        tint.source_type().value().copied(),
        Some("FixtureTint"),
        "the source stores this concrete tint type"
    );
    let [field] = tint.additional_fields().roots() else {
        return Err("one authored tint member".into());
    };
    assert_eq!(
        field.name().value().copied(),
        Some("factor"),
        "unknown tint members remain reachable"
    );
    assert_eq!(
        row.tint_override().origin().contains_encrypted_source(),
        Some(true),
        "the tint pointer has its own encryption marker"
    );
    Ok(())
}

fn assert_effects<'a, 'source>(
    slots: &'a [SourcedValue<ConsumableEffect<'source>>],
    inventory: &SourceInventory,
) -> TestResult<&'a ConsumableEffect<'source>> {
    let [status, health, empty, buff, resource, gas, extension] = slots else {
        return Err("seven physical effect slots".into());
    };
    for slot in slots {
        visit_leaf(slot, inventory)?;
    }
    assert_eq!(
        empty.state(),
        SourceValueState::Null,
        "the null effect slot remains in-place"
    );
    let status = status.value().ok_or("status effect")?;
    let health = health.value().ok_or("health effect")?;
    let buff = buff.value().ok_or("buff effect")?;
    let resource = resource.value().ok_or("resource effect")?;
    let gas = gas.value().ok_or("gas effect")?;
    let extension = extension.value().ok_or("authored effect")?;
    assert!(
        matches!(status.parameters(), ModifyActorStatus { .. }),
        "status variant"
    );
    assert!(
        matches!(resource.parameters(), Resource { .. }),
        "resource variant"
    );
    assert!(
        matches!(extension.parameters(), Authored),
        "extension variant"
    );
    let Health { health_change, .. } = health.parameters() else {
        return Err("health variant".into());
    };
    assert_eq!(
        health_change.value().map(ResourceFloat32::raw_bits),
        Some((-0.0_f32).to_bits()),
        "the stored negative zero survives"
    );
    assert_eq!(
        health_change.origin().contains_encrypted_source(),
        Some(true),
        "the change has direct field provenance"
    );
    assert_buff_and_gas(buff, gas)?;
    Ok(health)
}

fn assert_buff_and_gas(buff: &ConsumableEffect<'_>, gas: &ConsumableEffect<'_>) -> TestResult {
    let AddBuffEffect {
        buff_duration_override,
        ..
    } = buff.parameters()
    else {
        return Err("buff variant".into());
    };
    let duration = buff_duration_override
        .value()
        .ok_or("indefinite duration owner")?;
    assert_eq!(
        duration.source().source_type().value().copied(),
        Some("BuffDurationOverrideIndefiniteS42"),
        "the concrete indefinite override type survives"
    );
    let Gas { gas_mass, .. } = gas.parameters() else {
        return Err("gas variant".into());
    };
    let mass = gas_mass.value().ok_or("gas mass")?;
    let linked = mass.gas().value().ok_or("original gas Reference")?;
    assert!(
        matches!(linked.status(), DefinitionStatus::Dangling),
        "gas Reference remains dangling"
    );
    Ok(())
}
