// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogPublicationMode;
use p4kx_extraction::{DataForge, Value as Raw};
use serde_json::{Value, json};

use super::{DcbOrigin, TestResult, body, catalog, fixture, origins, proof, rows, source};

const SOURCE_BITS: u32 = 0x7fc0_4567;

#[test]
fn weapon_magazine_slots_publish_nonfinite_original_condition_numbers() -> TestResult {
    let builder = fixture::heat_magazine_database(f32::from_bits(SOURCE_BITS));
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let tree = source::tree(&forge, source::PERSONAL_INDEX)?;
    let weapon = source::weapon(&forge, &tree)?;
    let [slot] = weapon
        .secondary
        .ok_or("one original ordered secondary slot")?
    else {
        return Err("one original ordered secondary slot".into());
    };
    let selected = original_number(slot)?;
    let Raw::Single(number) = selected else {
        return Err("physical original Single threshold".into());
    };
    assert_eq!(
        number.to_bits(),
        SOURCE_BITS,
        "the original nonfinite token survives DCB decoding"
    );

    let catalog = catalog(
        &builder,
        DcbOrigin::Encrypted,
        CatalogPublicationMode::Normal,
    )?;
    let [_, row] = rows(&catalog)?;
    let condition = proof::at(row, "/secondaryAmmoContainers/value/0/value/condition")?;
    let node = wire_number(condition)?;
    let carrier = proof::present(
        node,
        "value",
        &json!({"type": "number", "value": {"type": "f32", "rawBits": format!("{SOURCE_BITS:08x}"), "finite": null}}),
        Some(true),
    )?;
    origins::exact(
        &catalog,
        carrier,
        &[body(&tree, selected, "$value", "payload")?],
    )?;
    assert_eq!(
        proof::at(&catalog, "/exportHealth/status")?,
        "complete",
        "the exact retained raw token qualifies for ordinary catalog publication"
    );
    proof::closure(&catalog)
}

fn original_number<'read, 'source>(slot: &'read Raw<'source>) -> TestResult<&'read Raw<'source>> {
    let slot = source::structure(slot)?;
    let condition = source::structure(slot.field("condition").ok_or("original condition")?)?;
    assert_eq!(
        condition.type_name(),
        "SWeaponConditionAll",
        "actual array-owning condition"
    );
    let Some(Raw::Array(children)) = condition.field("conditions") else {
        return Err("original strong pointer array".into());
    };
    let [heat, sibling] = children.as_ref() else {
        return Err("two actual ordered conditions".into());
    };
    let heat = source::structure(heat)?;
    assert_eq!(
        heat.type_name(),
        "SWeaponConditionHeatLevel",
        "first actual child"
    );
    assert_eq!(
        source::structure(sibling)?.type_name(),
        "SWeaponConditionIsHeldEntity",
        "distinct actual second child"
    );
    let comparer = source::structure(heat.field("comparer").ok_or("actual comparer")?)?;
    assert_eq!(
        comparer.type_name(),
        "SFloatNumberComparer",
        "original inline comparer"
    );
    comparer
        .field("value")
        .ok_or_else(|| "actual comparer Single".into())
}

fn wire_number(condition: &Value) -> TestResult<&Value> {
    let arena = proof::rows(condition, "/value/values")?;
    let root = node(arena, proof::at(condition, "/value/roots/0/value")?)?;
    let children = node(arena, proof::at(root, "/value/value/fields/0/value")?)?;
    let [first, second] = proof::rows(children, "/value/value/elements")? else {
        return Err("both actual ordered child links".into());
    };
    let sibling = node(arena, second)?;
    assert_eq!(
        proof::at(sibling, "/value/value/sourceType/value")?,
        "SWeaponConditionIsHeldEntity",
        "the second branch survives after closing the nested first branch"
    );
    let heat = node(arena, first)?;
    let comparer = node(arena, proof::at(heat, "/value/value/fields/0/value")?)?;
    let value = proof::at(comparer, "/value/value/fields/1")?;
    proof::present(value, "name", &json!("value"), Some(true))?;
    node(arena, proof::at(value, "/value")?)
}

fn node<'catalog>(arena: &'catalog [Value], reference: &Value) -> TestResult<&'catalog Value> {
    let index = proof::at(reference, "/valueIndex")?
        .as_str()
        .ok_or("arena-local decimal index")?
        .parse::<usize>()?;
    arena
        .get(index)
        .ok_or_else(|| "closed local arena reference".into())
}
