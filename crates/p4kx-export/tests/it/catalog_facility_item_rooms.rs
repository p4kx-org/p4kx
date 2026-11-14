// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DataForgeEntry, Value as Original};
use serde_json::{Value, json};

use super::{TestResult, build, fixture, interior_rows, proof};

#[test]
fn catalog_joins_item_rooms_to_each_exact_resolved_attachment() -> TestResult {
    let (_folder, archive) = fixture::archive(fixture::Case::ItemRooms)?;
    let slot = verify_original_room_component(&archive)?;
    let catalog = build(&archive)?;
    let placements = interior_rows(&catalog)?;
    assert_eq!(
        placements.len(),
        4,
        "two original SOC Entity slots occur once for each of two resolved attachment paths"
    );
    let [definition] = proof::rows(&catalog, "/declarations/itemRoomAssociations/rows")? else {
        return Err("one original item RoomAssociation definition".into());
    };
    proof::present(definition, "ownerRecordIndex", &json!(2), Some(false))?;
    proof::present(definition, "componentOrdinal", &json!(slot), Some(false))?;
    proof::present(
        definition,
        "objectContainer",
        &json!(fixture::ROOM),
        Some(false),
    )?;
    let coverage = proof::rows(&catalog, "/placements/facilityInteriorCoverage")?;
    assert_eq!(
        coverage.len(),
        2,
        "each actual attachment has its own room coverage"
    );
    for (placement, expected) in placements.iter().zip([0, 0, 1, 1]) {
        assert_attachment(proof::at(placement, "/source/room")?, slot, expected)?;
    }
    for (row, expected) in coverage.iter().zip([0, 1]) {
        assert_attachment(proof::at(row, "/room")?, slot, expected)?;
    }
    jsonschema::validator_for(&p4kx_export::catalog_schema())?
        .validate(&catalog)
        .map_err(|error| format!("schema: {error}"))?;
    proof::closure(&catalog)
}

fn assert_attachment(room: &Value, slot: usize, port: u32) -> TestResult {
    assert_eq!(
        room["kind"], "item",
        "the original declaration is a scalar item container"
    );
    let owner = proof::at(room, "/owner")?;
    proof::present(owner, "ownerRecordIndex", &json!(2), Some(false))?;
    proof::present(owner, "componentOrdinal", &json!(slot), Some(false))?;
    proof::present(
        room,
        "loadoutPosition",
        &json!({"loadoutOrdinal": 0, "portOrdinals": [port]}),
        Some(false),
    )?;
    Ok(())
}

fn verify_original_room_component(archive: &Archive) -> TestResult<usize> {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let item = forge
        .record(2)
        .ok_or("actual attached Item Record")?
        .tree()?;
    let Some(Original::Array(components)) = item.field("Components") else {
        return Err("physical item Components array".into());
    };
    let (slot, component) = components
        .iter()
        .enumerate()
        .find_map(|(slot, value)| match value {
            Original::Struct(component)
                if component.type_name() == "SObjectContainerComponentParams" =>
            {
                Some((slot, component))
            }
            _ => None,
        })
        .ok_or("actual scalar room component")?;
    assert!(
        matches!(component.field("objectContainer"), Some(Original::String(path)) if *path == fixture::ROOM),
        "the attached item names the real fixture SOCpak through its original scalar field"
    );
    Ok(slot)
}
