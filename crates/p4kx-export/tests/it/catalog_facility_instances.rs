// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_facility_instance_fixture.rs"]
mod fixture;

#[test]
fn ship_facility_instances_preserve_repeated_nested_and_docked_loadout_positions() -> TestResult {
    let (_directory, archive) = common::open(&fixture::database(), DcbOrigin::Encrypted)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let ship = loadout(&catalog, fixture::SHIP)?;
    let docked = loadout(&catalog, fixture::DOCKED)?;
    assert_eq!(
        proof::at(ship, "/ports/3/dockedVehicle")?,
        true,
        "resolved source stops at the docked vehicle"
    );
    assert_eq!(
        proof::at(ship, "/ports/4/unresolved")?,
        "dangling_reference",
        "unresolved original placement evidence remains in the Loadout"
    );
    assert_eq!(
        proof::at(docked, "/ports/0/itemGuid")?,
        &common::guid(fixture::SEAT),
        "the separately selected vehicle still resolves its own facilities"
    );

    let instances = proof::rows(&catalog, "/placements/facilityInstances/rows")?;
    check_ship_instances(instances)?;
    check_docked_instances(instances)?;
    validate(&catalog)
}

#[test]
fn facility_path_keeps_encrypted_ancestor_separate_from_clear_child_and_definition() -> TestResult {
    let (_directory, archive) = fixture::encrypted_xml_archive()?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let rows = proof::rows(&catalog, "/placements/facilityInstances/rows")?;
    let ship_rows = instances_of(rows, fixture::SHIP);
    let [storage, nested] = ship_rows.as_slice() else {
        return Err("the selected XML deck and its nested seat".into());
    };
    let storage_source = proof::at(storage, "/source")?;
    let nested_source = proof::at(nested, "/source")?;
    let parent_uses = proof::rows(storage_source, "/position/sourceUsesRefs")?;
    let nested_uses = proof::rows(nested_source, "/position/sourceUsesRefs")?;
    assert!(
        parent_uses.iter().all(|used| nested_uses.contains(used)),
        "the nested placement uses the complete ancestor path"
    );
    assert!(
        nested_uses.len() > parent_uses.len(),
        "the child adds its own original selectors"
    );
    assert_eq!(
        proof::at(nested_source, "/position/containsEncryptedSource")?,
        true,
        "the position was selected through encrypted ancestor XML"
    );
    proof::present(
        proof::at(nested_source, "/attachedItem")?,
        "targetRecordIndex",
        &json!(1),
        Some(false),
    )?;
    proof::present(
        proof::at(nested, "/configuration/owner")?,
        "ownerGuid",
        &json!(common::guid(fixture::SEAT)),
        Some(false),
    )?;
    validate(&catalog)
}

fn check_ship_instances(instances: &[Value]) -> TestResult {
    let ship_rows = instances_of(instances, fixture::SHIP);
    let [left, right, storage, nested] = ship_rows.as_slice() else {
        return Err("two direct seats, one storage placement, and one nested seat".into());
    };
    check_seat(left, &json!([0]))?;
    check_seat(right, &json!([1]))?;
    check_seat(nested, &json!([2, 0]))?;
    assert_eq!(
        proof::at(storage, "/source/position/value/portOrdinals")?,
        &json!([2]),
        "one storage instance at the deck port"
    );
    assert_eq!(
        proof::at(storage, "/configuration/kind")?,
        "inventory",
        "the occurrence links to the canonical storage configuration"
    );
    proof::present(
        proof::at(storage, "/configuration/owner")?,
        "ownerGuid",
        &json!(common::guid(fixture::DECK)),
        Some(true),
    )?;
    assert_ne!(
        proof::at(
            left,
            "/source/attachedItem/targetRecordIndex/sourceUsesRefs"
        )?,
        proof::at(
            right,
            "/source/attachedItem/targetRecordIndex/sourceUsesRefs"
        )?,
        "repeated instances keep independent original attachment uses"
    );
    Ok(())
}

fn check_docked_instances(instances: &[Value]) -> TestResult {
    let docked_rows = instances_of(instances, fixture::DOCKED);
    let [root, seat] = docked_rows.as_slice() else {
        return Err("docked vehicle has its own root and child seat".into());
    };
    assert_eq!(
        proof::at(root, "/source/position/value/portOrdinals")?,
        &json!([]),
        "the separately selected root has no host port"
    );
    assert_eq!(
        proof::at(root, "/source/attachedItem")?,
        &Value::Null,
        "root facilities have no Loadout attachment"
    );
    check_seat(seat, &json!([0]))
}

fn validate(catalog: &Value) -> TestResult {
    jsonschema::validator_for(&p4kx_export::catalog_schema())?
        .validate(catalog)
        .map_err(|error| format!("schema: {error}"))?;
    proof::closure(catalog)
}

fn check_seat(row: &Value, position: &Value) -> TestResult {
    assert_eq!(
        proof::at(row, "/source/kind")?,
        "loadout",
        "the occurrence is selected through the Loadout"
    );
    assert_eq!(
        proof::at(row, "/source/position/value/portOrdinals")?,
        position,
        "the whole original port path is retained"
    );
    assert_eq!(
        proof::at(row, "/configuration/kind")?,
        "facility",
        "the occurrence links to a physical seat component"
    );
    let owner = proof::at(row, "/configuration/owner")?;
    proof::present(
        owner,
        "ownerGuid",
        &json!(common::guid(fixture::SEAT)),
        Some(true),
    )?;
    proof::present(owner, "ownerRecordIndex", &json!(1), Some(true))?;
    proof::present(owner, "componentOrdinal", &json!(1), Some(true))?;
    let attachment = proof::at(row, "/source/attachedItem")?;
    proof::present(attachment, "targetRecordIndex", &json!(1), Some(true))?;
    Ok(())
}

fn loadout(catalog: &Value, guid: u128) -> TestResult<&Value> {
    let guid = json!(common::guid(guid));
    proof::rows(catalog, "/loadouts")?
        .iter()
        .find(|row| row.get("entityGuid") == Some(&guid))
        .ok_or_else(|| "actual synthetic ship loadout".into())
}

fn instances_of(instances: &[Value], guid: u128) -> Vec<&Value> {
    let guid = json!(common::guid(guid));
    instances
        .iter()
        .filter(|row| row.pointer("/shipGuid/value") == Some(&guid))
        .collect()
}
