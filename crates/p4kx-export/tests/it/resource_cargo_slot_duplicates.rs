// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{DataForgeEntry, Value as Raw};
use serde_json::{Value, json};

use crate::catalog_crafting_support;
use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::resource_cargo_slot_fixture::{self as fixture, FIRST, RESOURCE, SECOND};
use crate::resource_cargo_slot_origins;

#[test]
fn duplicate_cargo_slot_names_keep_distinct_targets_and_original_local_positions() -> TestResult {
    let (_folder, archive) = fixture::open()?;
    let input = DataForgeEntry::read(&archive)?;
    assert_original_slots(&input)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog_from(&archive, &input, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let rows = common::at(&catalog, "/definitions/resourceTypes/rows")?
        .as_array()
        .ok_or("Resource rows")?;
    assert_eq!(
        rows.len(),
        1,
        "the one original Resource owner remains selected"
    );
    let row = rows.first().ok_or("actual Resource row")?;
    assert_eq!(
        common::at(row, "/guid/value")?,
        &json!(catalog_crafting_support::text(RESOURCE)),
        "the row belongs to physical Record0"
    );
    let slots = common::at(row, "/defaultCargoContainers/value")?
        .as_array()
        .ok_or("actual cargo slots")?;
    assert_eq!(
        slots.len(),
        2,
        "both original declarations survive even when their names are equal"
    );
    assert_slot_values(slots)?;
    resource_cargo_slot_origins::assert_slots(&catalog, slots)?;
    assert_eq!(
        common::at(&catalog, "/exportHealth/status")?,
        &json!("complete"),
        "valid duplicate declarations are not globally rejected or turned into projection loss"
    );
    Ok(())
}

fn assert_original_slots(input: &DataForgeEntry) -> TestResult {
    let forge = input.open()?;
    let tree = forge.record(0).ok_or("original Resource Record")?.tree()?;
    assert_eq!(
        tree.record_index(),
        0,
        "the source owner is fixed before export"
    );
    let Some(Raw::Struct(slots)) = tree.field("defaultCargoContainers") else {
        return Err("original cargo structure".into());
    };
    assert_eq!(
        slots.fields().len(),
        2,
        "the canonical source retains both physical declaration positions"
    );
    for (ordinal, (slot, (index, guid))) in slots
        .fields()
        .iter()
        .zip([(1_u32, FIRST), (2_u32, SECOND)])
        .enumerate()
    {
        assert_eq!(
            slot.name(),
            "oneSCU",
            "both original names are equal at local ordinal{ordinal}"
        );
        let Raw::Reference(target) = slot.value() else {
            return Err("original typed cargo Reference".into());
        };
        assert!(
            target.location().is_none(),
            "the slot points to a whole Record, not an owned instance"
        );
        assert_eq!(
            target.record_index(),
            index,
            "physical slot{ordinal} has its own original target Record"
        );
        assert_eq!(
            target.guid().to_string(),
            catalog_crafting_support::text(guid),
            "distinct GUIDs exist in the original values before export"
        );
    }
    Ok(())
}

fn assert_target(slot: &Value, target_index: u32, guid: u128) -> TestResult {
    assert_eq!(
        common::at(slot, "/state")?,
        &json!("present"),
        "the original slot is present"
    );
    assert_eq!(
        common::at(slot, "/value/slot/value")?,
        &json!("oneSCU"),
        "the repeated authored name is not renamed or deduplicated"
    );
    let reference = common::at(slot, "/value/entityClass")?;
    assert_eq!(
        common::at(reference, "/state")?,
        &json!("present"),
        "both actual references are readable"
    );
    assert_eq!(
        common::at(reference, "/value/guid/value")?,
        &json!(catalog_crafting_support::text(guid)),
        "each list position keeps its own current Field.value(), not a first-by-name lookup"
    );
    assert_eq!(
        common::at(reference, "/value/targetRecordIndex/value")?,
        &json!(target_index),
        "target-header evidence belongs to that exact Reference"
    );
    assert_eq!(
        common::at(reference, "/value/targetType/value")?,
        &json!("EntityClassDefinition"),
        "the target type is independently resolved"
    );
    assert_eq!(
        common::at(reference, "/value/status")?,
        &json!("present"),
        "the genuine target header exists"
    );
    Ok(())
}

fn assert_slot_values(slots: &[Value]) -> TestResult {
    for (slot, (index, guid)) in slots.iter().zip([(1_u32, FIRST), (2_u32, SECOND)]) {
        assert_target(slot, index, guid)?;
    }
    Ok(())
}
